use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{ensure, Context, Result};
use cargo_metadata::Message;
use cargo_packager::{config::Binary, Config, PackageFormat};
use serde_json::json;

use crate::config::{Format, InstallMode, Plan};
use crate::{native, output, resources};

pub fn run(plan: &Plan) -> Result<()> {
    fs::create_dir_all(&plan.out_dir)?;
    // Refuse concurrent writers rather than interleaving two package manifests.
    let lock_path = plan.out_dir.join(".cargo-tgui.lock");
    let lock_file = fs::OpenOptions::new().write(true).create_new(true).open(&lock_path)
        .with_context(|| format!("cannot acquire {}; another bundle may be running (remove a stale lock after an interrupted run)", lock_path.display()))?;
    struct Lock(PathBuf, Option<fs::File>);
    impl Drop for Lock {
        fn drop(&mut self) {
            self.1.take();
            let _ = fs::remove_file(&self.0);
        }
    }
    let _lock = Lock(lock_path, Some(lock_file));
    let stage = tempfile::Builder::new()
        .prefix(".tgui-stage-")
        .tempdir_in(&plan.out_dir)?;
    let root = stage.path();
    let binaries = root.join("bin");
    let libraries = root.join("libraries");
    let packaged = root.join("packages");
    fs::create_dir_all(&binaries)?;
    fs::create_dir_all(&packaged)?;
    let built = build(plan)?;
    let executable = binaries.join(
        built
            .file_name()
            .context("Cargo artifact has no file name")?,
    );
    resources::copy(&built, &executable)?;
    let closure = native::collect(
        &built,
        &plan.target,
        &plan.os,
        &plan.config.library_search_paths,
    )?;
    eprintln!(
        "Collected {} redistributable libraries",
        closure.libraries.len()
    );
    native::stage(&closure, &executable, &libraries, &plan.os)?;
    let mut config = backend_config(plan, &binaries, &packaged)?;

    let mut resources = Vec::new();
    for file in &plan.resource_files {
        let staged = root.join("resources").join(&file.target);
        resources::copy(&file.source, &staged)?;
        resources.push(json!({"src": staged, "target": file.target}));
    }
    let dependency_manifest = root.join("tgui-native-dependencies.json");
    fs::write(&dependency_manifest, serde_json::to_vec_pretty(&closure)?)?;
    resources.push(json!({"src": dependency_manifest, "target": "tgui-native-dependencies.json"}));
    if plan.os == "windows" {
        for library in &closure.libraries {
            ensure!(
                !plan.resource_files.iter().any(|f| {
                    let target = f.target.to_string_lossy().replace('\\', "/").to_lowercase();
                    let name = library.name.to_lowercase();
                    target == name || target.starts_with(&format!("{name}/"))
                }),
                "resource conflicts with DLL {}",
                library.name
            );
            ensure!(
                !library
                    .name
                    .eq_ignore_ascii_case(&format!("{}.exe", plan.binary)),
                "DLL conflicts with main executable"
            );
            resources.push(json!({"src": libraries.join(&library.name), "target": library.name}));
        }
    }
    config.resources = Some(serde_json::from_value(json!(resources))?);

    if plan.os == "macos" {
        let macos = config.macos.as_mut().context("missing macOS settings")?;
        macos.frameworks = Some(
            closure
                .libraries
                .iter()
                .map(|lib| libraries.join(&lib.name).to_string_lossy().into_owned())
                .collect(),
        );
    }
    if plan.os == "windows" {
        let ico = plan
            .config
            .icons
            .iter()
            .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ico")))
            .cloned();
        let ico = match ico {
            Some(ico) => ico,
            None => {
                let icon = plan
                    .config
                    .icons
                    .iter()
                    .find(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")))
                    .context("Windows requires a PNG or ICO icon")?;
                let destination = root.join("installer.ico");
                image::open(icon)?
                    .resize(256, 256, image::imageops::FilterType::Lanczos3)
                    .save_with_format(&destination, image::ImageFormat::Ico)?;
                destination
            }
        };
        config
            .nsis
            .as_mut()
            .context("missing NSIS settings")?
            .installer_icon = Some(ico);
    }
    if plan.formats.contains(&Format::Deb) {
        let depends = native::deb_dependencies(&built, &plan.config.linux.depends)?;
        config.deb = Some(serde_json::from_value(
            json!({"depends": depends, "packageName": plan.binary.replace('_', "-")}),
        )?);
    }
    if plan.formats.contains(&Format::Appimage) {
        let mut files = serde_json::Map::new();
        for library in &closure.libraries {
            files.insert(
                libraries.join(&library.name).to_string_lossy().into_owned(),
                json!(format!("usr/lib/{}", library.name)),
            );
        }
        config.appimage = Some(serde_json::from_value(
            json!({"files": files, "excludedLibs": ["libc.so.*", "libm.so.*", "libdl.so.*", "libpthread.so.*", "librt.so.*", "libGL.so.*", "libEGL.so.*", "libGLX.so.*", "libGLdispatch.so.*", "libvulkan.so.*", "libcuda.so.*", "libnvidia-*"]}),
        )?);
    }

    let mut paths = Vec::new();
    if plan.os == "macos" {
        // Build and sign the app before constructing the disk image. Using hdiutil
        // here avoids the backend's implicit rebuild and optional notarization flow.
        config.formats = Some(vec![PackageFormat::App]);
        let app = cargo_packager::package(&config)?
            .into_iter()
            .flat_map(|p| p.paths)
            .next()
            .context("backend produced no app bundle")?;
        sign_app(&app, &plan.binary, &closure)?;
        if plan.formats.contains(&Format::Dmg) {
            paths.push(create_dmg(plan, &app, root, &packaged)?);
        }
        if plan.formats.contains(&Format::App) {
            let archive = packaged.join(format!(
                "{}-{}-{}.app.tar.gz",
                plan.package_name, plan.version, plan.target
            ));
            let encoder = flate2::write::GzEncoder::new(
                fs::File::create(&archive)?,
                flate2::Compression::default(),
            );
            let mut tar = tar::Builder::new(encoder);
            tar.append_dir_all(app.file_name().context("app has no name")?, &app)?;
            tar.into_inner()?.finish()?.sync_all()?;
            paths.push(archive);
            paths.push(app);
        }
    } else {
        for result in cargo_packager::package(&config).context("installer backend failed")? {
            paths.extend(result.paths);
        }
    }
    ensure!(!paths.is_empty(), "backend produced no artifacts");
    let mut artifacts = Vec::new();
    let mut sums = String::new();
    for path in &paths {
        ensure!(
            path.exists(),
            "backend reported missing output {}",
            path.display()
        );
        if path.is_dir() {
            artifacts.push(json!({"path": path.file_name().context("artifact has no name")?.to_string_lossy(), "kind": "directory"}));
        } else {
            let hash = resources::sha256(path)?;
            let name = path
                .file_name()
                .context("artifact has no name")?
                .to_string_lossy();
            sums.push_str(&format!("{hash}  {name}\n"));
            artifacts
                .push(json!({"path": name, "sha256": hash, "size": fs::metadata(path)?.len()}));
        }
    }
    let manifest = json!({"schema-version": 1, "tool-version": env!("CARGO_PKG_VERSION"), "backend-version": "0.11.8", "plan": plan, "native-dependencies": closure, "artifacts": artifacts});
    fs::write(
        packaged.join("bundle-manifest.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    fs::write(packaged.join("SHA256SUMS"), sums)?;
    paths.extend([
        packaged.join("bundle-manifest.json"),
        packaged.join("SHA256SUMS"),
    ]);
    publish_outputs(&paths, &plan.out_dir)?;
    eprintln!("Bundle complete: {}", plan.out_dir.display());
    Ok(())
}

fn build(plan: &Plan) -> Result<PathBuf> {
    let mut child = Command::new("cargo")
        .args(&plan.build_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("start Cargo build")?;
    let stdout = child.stdout.take().context("Cargo stdout unavailable")?;
    let mut artifact = None;
    let mut parse_error = None;
    // Always drain and wait, even if a message is malformed: no orphaned Cargo process.
    for message in Message::parse_stream(BufReader::new(stdout)) {
        match message {
            Ok(Message::CompilerArtifact(a))
                if a.package_id.to_string() == plan.package_id
                    && a.target.name == plan.binary
                    && a.target.kind.iter().any(|k| k == "bin") =>
            {
                if let Some(path) = a.executable {
                    artifact = Some(path.into_std_path_buf());
                }
            }
            Ok(Message::CompilerMessage(m)) => {
                if let Some(rendered) = m.message.rendered {
                    eprint!("{rendered}");
                }
            }
            Err(error) => parse_error = Some(error),
            _ => (),
        }
    }
    let status = child.wait()?;
    ensure!(status.success(), "Cargo build failed ({status})");
    if let Some(error) = parse_error {
        return Err(error.into());
    }
    artifact.context("Cargo did not report the selected executable")
}

fn backend_config(plan: &Plan, binaries: &Path, destination: &Path) -> Result<Config> {
    let formats: Vec<_> = plan
        .formats
        .iter()
        .map(|format| match format {
            Format::Nsis => PackageFormat::Nsis,
            Format::App => PackageFormat::App,
            Format::Dmg => PackageFormat::Dmg,
            Format::Deb => PackageFormat::Deb,
            Format::Appimage => PackageFormat::AppImage,
        })
        .collect();
    let install_mode = match plan.config.windows.install_mode {
        InstallMode::CurrentUser => "currentUser",
        InstallMode::PerMachine => "perMachine",
    };
    let config: Config = serde_json::from_value(json!({
        "productName": plan.config.product_name, "version": plan.version,
        "identifier": plan.config.identifier, "publisher": plan.config.publisher,
        "description": plan.description, "authors": plan.authors,
        "outDir": destination, "binariesDir": binaries, "targetTriple": plan.target,
        "binaries": [Binary::new(&plan.binary).main(true)],
        "icons": plan.config.icons.iter().map(|p| glob::Pattern::escape(&p.to_string_lossy())).collect::<Vec<_>>(), "formats": formats,
        "category": "Utility",
        "windows": {"allowDowngrades": false},
        "nsis": {"installMode": install_mode, "languages": plan.config.windows.languages},
        "macos": {"minimumSystemVersion": plan.config.macos.minimum_system_version}
    }))
    .context("construct backend configuration")?;
    Ok(config)
}

fn sign_app(app: &Path, binary: &str, closure: &native::Closure) -> Result<()> {
    for library in &closure.libraries {
        output(
            Command::new("codesign")
                .args(["--force", "--sign", "-", "--timestamp=none"])
                .arg(app.join("Contents/Frameworks").join(&library.name)),
        )?;
    }
    output(
        Command::new("codesign")
            .args(["--force", "--sign", "-", "--timestamp=none"])
            .arg(app.join("Contents/MacOS").join(binary)),
    )?;
    output(
        Command::new("codesign")
            .args(["--force", "--sign", "-", "--timestamp=none"])
            .arg(app),
    )?;
    output(
        Command::new("codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(app),
    )?;
    Ok(())
}

fn create_dmg(plan: &Plan, app: &Path, stage: &Path, packages: &Path) -> Result<PathBuf> {
    let contents = stage.join("dmg");
    fs::create_dir(&contents)?;
    output(
        Command::new("ditto")
            .arg(app)
            .arg(contents.join(app.file_name().context("app has no name")?)),
    )?;
    #[cfg(unix)]
    std::os::unix::fs::symlink("/Applications", contents.join("Applications"))?;
    let dmg = packages.join(format!(
        "{}-{}-{}.dmg",
        plan.package_name, plan.version, plan.target
    ));
    output(
        Command::new("hdiutil")
            .args(["create", "-ov", "-format", "UDZO", "-volname"])
            .arg(&plan.config.product_name)
            .arg("-srcfolder")
            .arg(&contents)
            .arg(&dmg),
    )?;
    Ok(dmg)
}

fn publish_outputs(paths: &[PathBuf], out_dir: &Path) -> Result<()> {
    // Only replace paths recorded by a previous successful run. An arbitrary --out-dir
    // must never authorize deletion of an unrelated directory or user file.
    let previous = out_dir.join("bundle-manifest.json");
    let mut owned = std::collections::BTreeSet::new();
    if previous.exists() {
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&previous)?)?;
        for item in value["artifacts"]
            .as_array()
            .context("invalid previous bundle manifest")?
        {
            let path = item["path"]
                .as_str()
                .context("invalid previous artifact path")?;
            resources::safe_relative(Path::new(path))?;
            ensure!(
                Path::new(path).components().count() == 1,
                "invalid previous artifact name"
            );
            owned.insert(path.to_owned());
        }
        owned.extend(["bundle-manifest.json".into(), "SHA256SUMS".into()]);
    }
    for path in paths {
        let name = path.file_name().context("output has no name")?;
        ensure!(
            !out_dir.join(name).exists() || owned.contains(name.to_string_lossy().as_ref()),
            "refusing to overwrite unrelated output {}",
            out_dir.join(name).display()
        );
    }
    for path in paths {
        let destination = out_dir.join(path.file_name().context("output has no name")?);
        if destination.is_dir() {
            fs::remove_dir_all(&destination)?;
        } else if destination.exists() {
            fs::remove_file(&destination)?;
        }
        fs::rename(path, destination)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publishing_cannot_remove_unrelated_files() {
        let dir = tempfile::tempdir().unwrap();
        let stage = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("app.dmg"), "user file").unwrap();
        fs::write(stage.path().join("app.dmg"), "new bundle").unwrap();
        assert!(publish_outputs(&[stage.path().join("app.dmg")], dir.path()).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("app.dmg")).unwrap(),
            "user file"
        );
    }
}
