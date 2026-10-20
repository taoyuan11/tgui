//! Native dependency inspection. Only staging copies are ever rewritten.
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, ensure, Context, Result};
use goblin::{mach::Mach, Object};
use serde::Serialize;

use crate::{output, resources};

#[derive(Clone, Debug, Serialize)]
pub struct Library {
    pub name: String,
    pub source: PathBuf,
    pub sha256: String,
    pub imports: Vec<Edge>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Edge {
    pub import: String,
    pub bundled_name: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct Closure {
    pub libraries: Vec<Library>,
    pub executable_imports: Vec<Edge>,
    pub system_libraries: BTreeSet<String>,
}

struct Info {
    imports: Vec<String>,
    rpaths: Vec<String>,
}

fn inspect(path: &Path, target: &str) -> Result<Info> {
    let bytes = fs::read(path).with_context(|| format!("read native binary {}", path.display()))?;
    match Object::parse(&bytes)
        .with_context(|| format!("parse native binary {}", path.display()))?
    {
        Object::PE(pe) => {
            ensure!(
                target == "x86_64-pc-windows-msvc" && pe.header.coff_header.machine == 0x8664,
                "wrong PE architecture: {}",
                path.display()
            );
            Ok(Info {
                imports: pe.libraries.iter().map(|s| s.to_string()).collect(),
                rpaths: vec![],
            })
        }
        Object::Elf(elf) => {
            ensure!(
                target == "x86_64-unknown-linux-gnu" && elf.header.e_machine == 62 && elf.is_64,
                "wrong ELF architecture: {}",
                path.display()
            );
            Ok(Info {
                imports: elf.libraries.iter().map(|s| s.to_string()).collect(),
                rpaths: elf
                    .runpaths
                    .iter()
                    .chain(elf.rpaths.iter())
                    .flat_map(|s| s.split(':').map(str::to_owned))
                    .collect(),
            })
        }
        Object::Mach(mach) => {
            ensure!(
                target.ends_with("apple-darwin"),
                "Mach-O binary supplied for {target}"
            );
            let cpu = if target.starts_with("aarch64") {
                0x0100000c
            } else {
                0x01000007
            };
            let get_info = |m: goblin::mach::MachO<'_>| -> Result<Info> {
                ensure!(
                    m.header.cputype == cpu,
                    "wrong Mach-O architecture: {}",
                    path.display()
                );
                Ok(Info {
                    imports: m
                        .libs
                        .iter()
                        .filter(|s| **s != "self" && Some(**s) != m.name)
                        .map(|s| s.to_string())
                        .collect(),
                    rpaths: m.rpaths.iter().map(|s| s.to_string()).collect(),
                })
            };
            match mach {
                Mach::Binary(m) => get_info(m),
                Mach::Fat(fat) => {
                    for (index, arch) in fat.iter_arches().enumerate() {
                        if arch?.cputype == cpu {
                            if let goblin::mach::SingleArch::MachO(m) = fat.get(index)? {
                                return get_info(m);
                            }
                        }
                    }
                    bail!("Mach-O has no slice for {target}: {}", path.display())
                }
            }
        }
        _ => bail!("unsupported native binary {}", path.display()),
    }
}

pub fn collect(
    executable: &Path,
    target: &str,
    os: &str,
    search_paths: &[PathBuf],
) -> Result<Closure> {
    let root = inspect(executable, target)?;
    let mut search = search_paths.to_vec();
    if let Some(paths) = std::env::var_os("PATH") {
        search.extend(std::env::split_paths(&paths));
    }
    // The same build environment can provide dynamically linked FFmpeg and the MSVC CRT.
    if let Some(root) = std::env::var_os("VCPKG_ROOT") {
        search.push(PathBuf::from(root).join("installed/x64-windows/bin"));
    }
    if let Some(root) = std::env::var_os("VCToolsRedistDir") {
        for entry in walkdir::WalkDir::new(PathBuf::from(root).join("x64"))
            .max_depth(2)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_dir() {
                search.push(entry.into_path());
            }
        }
    }
    let linux_paths = if os == "linux" {
        linux_resolutions(executable, search_paths)?
    } else {
        BTreeMap::new()
    };
    let root_rpaths = expand_rpaths(&root.rpaths, executable, executable);
    let mut pending = VecDeque::from([(
        executable.to_path_buf(),
        None::<String>,
        root_rpaths.clone(),
    )]);
    let mut libraries = BTreeMap::<String, Library>::new();
    let mut inspected = BTreeSet::new();
    let mut executable_imports = Vec::new();
    let mut system_libraries = BTreeSet::new();
    while let Some((source, library_key, inherited_rpaths)) = pending.pop_front() {
        if !inspected.insert((source.clone(), library_key.clone())) {
            continue;
        }
        let info = inspect(&source, target)?;
        let mut rpaths = expand_rpaths(&info.rpaths, &source, executable);
        rpaths.extend(inherited_rpaths);
        let mut edges = Vec::new();
        for import in info.imports {
            if is_system(&import, os) {
                system_libraries.insert(import.clone());
                edges.push(Edge {
                    import,
                    bundled_name: None,
                });
                continue;
            }
            let resolved = resolve(
                &import,
                &source,
                executable,
                &rpaths,
                &search,
                &linux_paths,
                os,
            )
            .with_context(|| {
                format!(
                    "unresolved dependency {import} imported by {}; configure library-search-paths",
                    source.display()
                )
            })?;
            // Windows OS DLLs may be named beyond the small API-set allowlist. Do not
            // distribute files from System32, except redistributable MSVC libraries.
            if os == "windows" && is_windows_system_path(&resolved, &import) {
                system_libraries.insert(import.clone());
                edges.push(Edge {
                    import,
                    bundled_name: None,
                });
                continue;
            }
            inspect(&resolved, target)?;
            let name = if os == "linux" {
                Path::new(&import).file_name()
            } else {
                resolved.file_name()
            }
            .context("dependency has no file name")?
            .to_str()
            .context("non-UTF-8 library name")?
            .to_owned();
            resources::safe_relative(Path::new(&name))?;
            let key = name.to_lowercase();
            let digest = resources::sha256(&resolved)?;
            if let Some(existing) = libraries.get(&key) {
                ensure!(
                    existing.name == name,
                    "library names differ only in case: {} and {name}",
                    existing.name
                );
                ensure!(
                    existing.sha256 == digest,
                    "different libraries have the same destination name {name}: {} and {}",
                    existing.source.display(),
                    resolved.display()
                );
            } else {
                libraries.insert(
                    key.clone(),
                    Library {
                        name: name.clone(),
                        source: resolved.clone(),
                        sha256: digest,
                        imports: vec![],
                    },
                );
            }
            pending.push_back((resolved, Some(key), rpaths.clone()));
            edges.push(Edge {
                import,
                bundled_name: Some(name),
            });
        }
        if let Some(key) = library_key {
            libraries
                .get_mut(&key)
                .context("missing dependency record")?
                .imports = edges;
        } else {
            executable_imports = edges;
        }
    }
    Ok(Closure {
        libraries: libraries.into_values().collect(),
        executable_imports,
        system_libraries,
    })
}

fn expand_rpaths(rpaths: &[String], loader: &Path, executable: &Path) -> Vec<PathBuf> {
    rpaths
        .iter()
        .map(|p| expand_tokens(p, loader, executable))
        .collect()
}
fn expand_tokens(path: &str, loader: &Path, executable: &Path) -> PathBuf {
    PathBuf::from(
        path.replace(
            "@loader_path",
            &loader.parent().unwrap_or(Path::new("/")).to_string_lossy(),
        )
        .replace(
            "@executable_path",
            &executable
                .parent()
                .unwrap_or(Path::new("/"))
                .to_string_lossy(),
        )
        .replace(
            "${ORIGIN}",
            &loader.parent().unwrap_or(Path::new("/")).to_string_lossy(),
        )
        .replace(
            "$ORIGIN",
            &loader.parent().unwrap_or(Path::new("/")).to_string_lossy(),
        ),
    )
}

fn resolve(
    import: &str,
    loader: &Path,
    executable: &Path,
    rpaths: &[PathBuf],
    search: &[PathBuf],
    linux: &BTreeMap<String, PathBuf>,
    os: &str,
) -> Result<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(rest) = import.strip_prefix("@rpath/") {
        candidates.extend(rpaths.iter().map(|p| p.join(rest)));
    } else {
        let expanded = expand_tokens(import, loader, executable);
        if expanded.is_absolute() {
            candidates.push(expanded);
        }
    }
    if let Some(parent) = loader.parent() {
        candidates.push(parent.join(import));
    }
    if let Some(parent) = executable.parent() {
        candidates.push(parent.join(import));
    }
    if let Some(resolved) = linux.get(import) {
        candidates.push(resolved.clone());
    }
    let name = Path::new(import)
        .file_name()
        .context("empty dependency name")?;
    candidates.extend(search.iter().map(|p| p.join(name)));
    if os == "windows" {
        if let Some(windir) = std::env::var_os("SystemRoot") {
            candidates.push(PathBuf::from(windir).join("System32").join(name));
        }
    }
    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate.canonicalize()?);
        }
        if os == "windows" {
            if let Some(parent) = candidate.parent() {
                if let Ok(entries) = fs::read_dir(parent) {
                    if let Some(found) = entries.filter_map(Result::ok).find(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .eq_ignore_ascii_case(&name.to_string_lossy())
                    }) {
                        return Ok(found.path().canonicalize()?);
                    }
                }
            }
        }
    }
    bail!("library not found: {import}")
}

fn is_windows_system_path(path: &Path, name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    if name.starts_with("vcruntime") || name.starts_with("msvcp") || name.starts_with("concrt") {
        return false;
    }
    std::env::var_os("SystemRoot")
        .and_then(|p| PathBuf::from(p).join("System32").canonicalize().ok())
        .is_some_and(|root| path.starts_with(root))
}

fn is_system(import: &str, os: &str) -> bool {
    match os {
        "macos" => import.starts_with("/usr/lib/") || import.starts_with("/System/Library/"),
        "windows" => {
            let name = import.to_ascii_lowercase();
            name.starts_with("api-ms-win-") || name.starts_with("ext-ms-win-")
        }
        _ => {
            // glibc, loaders and host GPU drivers must remain host-provided.
            let name = Path::new(import)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            [
                "libc.so.",
                "libm.so.",
                "libdl.so.",
                "libpthread.so.",
                "librt.so.",
                "libresolv.so.",
                "libutil.so.",
                "ld-linux-",
                "libGL.so.",
                "libEGL.so.",
                "libGLX.so.",
                "libGLdispatch.so.",
                "libvulkan.so.",
                "libcuda.so.",
                "libnvidia-",
            ]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        }
    }
}

fn linux_resolutions(executable: &Path, search: &[PathBuf]) -> Result<BTreeMap<String, PathBuf>> {
    let mut command = Command::new("ldd");
    command.arg(executable);
    if !search.is_empty() {
        let mut paths = search.to_vec();
        if let Some(existing) = std::env::var_os("LD_LIBRARY_PATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        command.env("LD_LIBRARY_PATH", std::env::join_paths(paths)?);
    }
    let text = output(&mut command)?;
    let mut paths = BTreeMap::new();
    for line in text.lines() {
        if let Some((name, rest)) = line.trim().split_once(" => ") {
            ensure!(
                !rest.starts_with("not found"),
                "ldd could not resolve {name}; configure the build environment before bundling"
            );
            let path = rest.rsplit_once(" (").map(|p| p.0).unwrap_or(rest).trim();
            if Path::new(path).is_absolute() {
                paths.insert(name.into(), path.into());
            }
        }
    }
    Ok(paths)
}

pub fn stage(closure: &Closure, executable: &Path, libraries_dir: &Path, os: &str) -> Result<()> {
    fs::create_dir_all(libraries_dir)?;
    for library in &closure.libraries {
        resources::copy(&library.source, &libraries_dir.join(&library.name))?;
    }
    if os == "macos" {
        rewrite_macho(executable, &closure.executable_imports, None)?;
        for library in &closure.libraries {
            rewrite_macho(
                &libraries_dir.join(&library.name),
                &library.imports,
                Some(&library.name),
            )?;
        }
    }
    Ok(())
}

fn rewrite_macho(path: &Path, edges: &[Edge], name: Option<&str>) -> Result<()> {
    // install_name_tool invalidates old signatures. The final bundle is signed inside out.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(permissions.mode() | 0o200);
        fs::set_permissions(path, permissions)?;
    }
    let mut cmd = Command::new("install_name_tool");
    let target = if cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else {
        "x86_64-apple-darwin"
    };
    let rpaths = inspect(path, target)?.rpaths;
    for rpath in &rpaths {
        cmd.args(["-delete_rpath", rpath]);
    }
    if let Some(name) = name {
        cmd.args(["-id", &format!("@rpath/{name}")]);
    }
    for edge in edges {
        if let Some(dependency) = &edge.bundled_name {
            let new = if name.is_some() {
                format!("@loader_path/{dependency}")
            } else {
                format!("@executable_path/../Frameworks/{dependency}")
            };
            cmd.args(["-change", &edge.import, &new]);
        }
    }
    if !rpaths.is_empty() || name.is_some() || edges.iter().any(|e| e.bundled_name.is_some()) {
        output(cmd.arg(path))?;
    }
    Ok(())
}

pub fn deb_dependencies(executable: &Path, extra: &[String]) -> Result<Vec<String>> {
    let directory = tempfile::tempdir()?;
    fs::create_dir(directory.path().join("debian"))?;
    fs::write(directory.path().join("debian/control"), "Source: tgui-bundle\nSection: utils\nPriority: optional\nMaintainer: tgui <noreply@example.com>\nStandards-Version: 4.6.2\n\nPackage: tgui-bundle\nArchitecture: any\nDescription: dependency inspection\n")?;
    let result = output(
        Command::new("dpkg-shlibdeps")
            .current_dir(directory.path())
            .arg("-O")
            .arg("-e")
            .arg(executable),
    )?;
    let depends = result
        .lines()
        .find_map(|l| l.strip_prefix("shlibs:Depends="))
        .context("dpkg-shlibdeps returned no Depends field")?;
    let mut depends: BTreeSet<String> = depends
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    depends.extend(extra.iter().cloned());
    Ok(depends.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_libraries_are_distinguished_from_ffmpeg_and_crt() {
        assert!(is_system("/usr/lib/libSystem.B.dylib", "macos"));
        assert!(!is_system("/opt/homebrew/lib/libavcodec.62.dylib", "macos"));
        assert!(is_system("api-ms-win-crt-runtime-l1-1-0.dll", "windows"));
        assert!(!is_system("VCRUNTIME140.dll", "windows"));
        assert!(is_system("libc.so.6", "linux"));
        assert!(!is_system("libavcodec.so.61", "linux"));
    }
    #[test]
    fn native_inspection_checks_architecture() {
        let exe = std::env::current_exe().unwrap();
        let wrong = if cfg!(windows) {
            "aarch64-apple-darwin"
        } else {
            "x86_64-pc-windows-msvc"
        };
        assert!(inspect(&exe, wrong).is_err());
    }
    #[test]
    fn loader_paths_resolve_without_the_working_directory() {
        assert_eq!(
            expand_tokens(
                "@loader_path/../lib/example.dylib",
                Path::new("/space dir/bin/plugin"),
                Path::new("/app/main")
            ),
            Path::new("/space dir/bin/../lib/example.dylib")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn different_libraries_cannot_share_a_bundle_destination() {
        let directory = tempfile::tempdir().unwrap();
        let mut libraries = Vec::new();
        for symbol in ["first", "second"] {
            let source = directory.path().join(symbol);
            fs::create_dir(&source).unwrap();
            fs::write(
                source.join("leaf.c"),
                format!("int {symbol}(void) {{ return 42; }}"),
            )
            .unwrap();
            let library = source.join("libleaf.dylib");
            output(
                Command::new("cc")
                    .current_dir(&source)
                    .args(["-dynamiclib", "leaf.c", "-o"])
                    .arg(&library)
                    .arg(format!("-Wl,-install_name,{}", library.display())),
            )
            .unwrap();
            libraries.push(library);
        }
        let executable = directory.path().join("app");
        fs::write(directory.path().join("main.c"), "extern int first(void); extern int second(void); int main(void) { return first() != second(); }").unwrap();
        output(
            Command::new("cc")
                .current_dir(directory.path())
                .arg("main.c")
                .args(&libraries)
                .arg("-o")
                .arg(&executable),
        )
        .unwrap();
        let target = if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        };
        assert!(collect(&executable, target, "macos", &[])
            .unwrap_err()
            .to_string()
            .contains("same destination name"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn transitive_libraries_relocate_and_missing_dependencies_fail() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source with spaces");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("leaf.c"), "int leaf(void) { return 42; }").unwrap();
        fs::write(
            source.join("parent.c"),
            "extern int leaf(void); int parent(void) { return leaf(); }",
        )
        .unwrap();
        fs::write(
            source.join("main.c"),
            "extern int parent(void); int main(void) { return parent() != 42; }",
        )
        .unwrap();
        output(Command::new("cc").current_dir(&source).args([
            "-dynamiclib",
            "leaf.c",
            "-o",
            "libleaf.dylib",
            "-Wl,-install_name,@rpath/libleaf.dylib",
        ]))
        .unwrap();
        output(Command::new("cc").current_dir(&source).args([
            "-dynamiclib",
            "parent.c",
            "-o",
            "libparent.dylib",
            "-L.",
            "-lleaf",
            "-Wl,-install_name,@rpath/libparent.dylib",
        ]))
        .unwrap();
        output(
            Command::new("cc")
                .current_dir(&source)
                .args(["main.c", "-o", "app", "-L.", "-lparent"])
                .arg(format!("-Wl,-rpath,{}", source.display())),
        )
        .unwrap();
        let target = if cfg!(target_arch = "aarch64") {
            "aarch64-apple-darwin"
        } else {
            "x86_64-apple-darwin"
        };
        let closure = collect(&source.join("app"), target, "macos", &[]).unwrap();
        assert_eq!(closure.libraries.len(), 2);
        let executable = directory.path().join("Installed.app/Contents/MacOS/app");
        resources::copy(&source.join("app"), &executable).unwrap();
        let libraries = directory.path().join("Installed.app/Contents/Frameworks");
        stage(&closure, &executable, &libraries, "macos").unwrap();
        for library in &closure.libraries {
            output(
                Command::new("codesign")
                    .args(["--force", "--sign", "-", "--timestamp=none"])
                    .arg(libraries.join(&library.name)),
            )
            .unwrap();
        }
        output(
            Command::new("codesign")
                .args(["--force", "--sign", "-", "--timestamp=none"])
                .arg(&executable),
        )
        .unwrap();
        fs::remove_file(source.join("libleaf.dylib")).unwrap();
        assert!(collect(&source.join("app"), target, "macos", &[])
            .unwrap_err()
            .to_string()
            .contains("unresolved dependency"));
        fs::remove_file(source.join("libparent.dylib")).unwrap();
        output(
            Command::new(&executable)
                .env_remove("DYLD_LIBRARY_PATH")
                .env_remove("DYLD_FALLBACK_LIBRARY_PATH"),
        )
        .unwrap();
        assert!(inspect(&executable, target).unwrap().rpaths.is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn elf_dependency_closure_uses_configured_search_directories() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source with spaces");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("leaf.c"), "int leaf(void) { return 42; }").unwrap();
        fs::write(
            source.join("parent.c"),
            "extern int leaf(void); int parent(void) { return leaf(); }",
        )
        .unwrap();
        fs::write(
            source.join("main.c"),
            "extern int parent(void); int main(void) { return parent() != 42; }",
        )
        .unwrap();
        output(Command::new("cc").current_dir(&source).args([
            "-shared",
            "-fPIC",
            "leaf.c",
            "-o",
            "libleaf.so",
            "-Wl,-soname,libleaf.so",
        ]))
        .unwrap();
        output(Command::new("cc").current_dir(&source).args([
            "-shared",
            "-fPIC",
            "parent.c",
            "-o",
            "libparent.so",
            "-L.",
            "-lleaf",
            "-Wl,-soname,libparent.so",
        ]))
        .unwrap();
        output(Command::new("cc").current_dir(&source).args([
            "main.c",
            "-o",
            "app",
            "-L.",
            "-lparent",
            "-Wl,-rpath-link,.",
        ]))
        .unwrap();
        let closure = collect(
            &source.join("app"),
            "x86_64-unknown-linux-gnu",
            "linux",
            &[source.clone()],
        )
        .unwrap();
        assert_eq!(closure.libraries.len(), 2);
        let destination = directory.path().join("installed");
        let executable = destination.join("app");
        resources::copy(&source.join("app"), &executable).unwrap();
        stage(&closure, &executable, &destination, "linux").unwrap();
        fs::remove_file(source.join("libleaf.so")).unwrap();
        assert!(collect(
            &source.join("app"),
            "x86_64-unknown-linux-gnu",
            "linux",
            &[source.clone()]
        )
        .is_err());
        fs::remove_file(source.join("libparent.so")).unwrap();
        output(Command::new(&executable).env("LD_LIBRARY_PATH", &destination)).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn pe_dependency_closure_survives_removing_build_dlls() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source with spaces");
        fs::create_dir(&source).unwrap();
        fs::write(
            source.join("leaf.c"),
            "__declspec(dllexport) int leaf(void) { return 42; }",
        )
        .unwrap();
        fs::write(source.join("parent.c"), "__declspec(dllimport) int leaf(void); __declspec(dllexport) int parent(void) { return leaf(); }").unwrap();
        fs::write(
            source.join("main.c"),
            "__declspec(dllimport) int parent(void); int main(void) { return parent() != 42; }",
        )
        .unwrap();
        output(Command::new("cl").current_dir(&source).args([
            "/nologo",
            "/LD",
            "leaf.c",
            "/Fe:leaf.dll",
            "/link",
            "/IMPLIB:leaf.lib",
        ]))
        .unwrap();
        output(Command::new("cl").current_dir(&source).args([
            "/nologo",
            "/LD",
            "parent.c",
            "leaf.lib",
            "/Fe:parent.dll",
            "/link",
            "/IMPLIB:parent.lib",
        ]))
        .unwrap();
        output(Command::new("cl").current_dir(&source).args([
            "/nologo",
            "main.c",
            "parent.lib",
            "/Fe:app.exe",
        ]))
        .unwrap();
        let closure = collect(
            &source.join("app.exe"),
            "x86_64-pc-windows-msvc",
            "windows",
            &[],
        )
        .unwrap();
        for name in ["leaf.dll", "parent.dll"] {
            assert!(closure
                .libraries
                .iter()
                .any(|l| l.name.eq_ignore_ascii_case(name)));
        }
        let destination = directory.path().join("installed");
        let executable = destination.join("app.exe");
        resources::copy(&source.join("app.exe"), &executable).unwrap();
        stage(&closure, &executable, &destination, "windows").unwrap();
        fs::remove_file(source.join("leaf.dll")).unwrap();
        assert!(collect(
            &source.join("app.exe"),
            "x86_64-pc-windows-msvc",
            "windows",
            &[]
        )
        .is_err());
        fs::remove_file(source.join("parent.dll")).unwrap();
        output(&mut Command::new(&executable)).unwrap();
    }
}
