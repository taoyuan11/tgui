use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use anyhow::{ensure, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::config::Resource;

#[derive(Clone, Debug, Serialize)]
pub struct File {
    pub source: PathBuf,
    pub target: PathBuf,
}

pub fn safe_relative(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty()
            && path.components().all(|p| matches!(p, Component::Normal(_))),
        "destination must be a nonempty relative path without '.' or '..': {}",
        path.display()
    );
    for part in path.components() {
        let part = part
            .as_os_str()
            .to_str()
            .context("resource names must be UTF-8")?;
        ensure!(
            !part
                .chars()
                .any(|c| c.is_control() || "<>:\"\\|?*".contains(c))
                && !part.ends_with(['.', ' ']),
            "non-portable resource name {part}"
        );
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        ensure!(
            ![
                "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
                "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8",
                "LPT9"
            ]
            .contains(&stem.as_str()),
            "reserved Windows file name {part}"
        );
    }
    Ok(())
}

pub fn expand(
    resources: &[Resource],
    licenses: &[PathBuf],
    binary: &str,
    os: &str,
) -> Result<Vec<File>> {
    let mut files = Vec::new();
    for resource in resources {
        safe_relative(&resource.target)?;
        let metadata = fs::symlink_metadata(&resource.src)
            .with_context(|| format!("missing resource {}", resource.src.display()))?;
        ensure!(
            !metadata.file_type().is_symlink(),
            "resource symlinks are not supported: {}",
            resource.src.display()
        );
        if metadata.is_dir() {
            for entry in WalkDir::new(&resource.src).sort_by_file_name() {
                let entry = entry?;
                ensure!(
                    !entry.file_type().is_symlink(),
                    "resource symlinks are not supported: {}",
                    entry.path().display()
                );
                if entry.file_type().is_file() {
                    files.push(File {
                        source: entry.path().canonicalize()?,
                        target: resource
                            .target
                            .join(entry.path().strip_prefix(&resource.src)?),
                    });
                }
            }
        } else {
            ensure!(
                metadata.is_file(),
                "resource must be a regular file or directory"
            );
            files.push(File {
                source: resource.src.canonicalize()?,
                target: resource.target.clone(),
            });
        }
    }
    for license in licenses {
        files.push(File {
            source: license.clone(),
            target: Path::new("licenses")
                .join(license.file_name().context("license file has no name")?),
        });
    }
    let mut destinations = BTreeSet::new();
    destinations.insert("tgui-native-dependencies.json".into());
    if os == "windows" {
        destinations.insert(format!("{}.exe", binary.to_lowercase()));
        destinations.insert("uninstall.exe".into());
    }
    for file in &files {
        safe_relative(&file.target)?;
        let key = file
            .target
            .to_string_lossy()
            .replace('\\', "/")
            .to_lowercase();
        ensure!(
            !destinations.iter().any(|other| other == &key
                || other.starts_with(&format!("{key}/"))
                || key.starts_with(&format!("{other}/"))),
            "resource destination collision: {}",
            file.target.display()
        );
        destinations.insert(key);
    }
    files.sort_by(|a, b| a.target.cmp(&b.target));
    Ok(files)
}

pub fn copy(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target.parent().context("copy destination has no parent")?)?;
    fs::copy(source, target)
        .with_context(|| format!("copy {} to {}", source.display(), target.display()))?;
    Ok(())
}

pub fn sha256(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_portable_and_cannot_escape() {
        for path in [
            "",
            ".",
            "..",
            "a/../b",
            "/tmp/out",
            "C:\\escape",
            "a\\..\\b",
            "CON.txt",
            "a.",
        ] {
            assert!(safe_relative(Path::new(path)).is_err(), "{path}");
        }
        safe_relative(Path::new("资源 files/test.txt")).unwrap();
    }

    #[test]
    fn expansion_checks_case_and_file_directory_collisions() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("resource");
        fs::write(&src, b"contents").unwrap();
        let first = Resource {
            src: src.clone(),
            target: "Assets/file".into(),
        };
        for target in ["assets/FILE", "Assets", "assets/file/child"] {
            let second = Resource {
                src: src.clone(),
                target: target.into(),
            };
            assert!(expand(&[first.clone(), second], &[], "app", "linux").is_err());
        }
        assert!(expand(
            &[Resource {
                src: src.clone(),
                target: "tgui-native-dependencies.json".into()
            }],
            &[],
            "app",
            "linux"
        )
        .is_err());
        assert!(expand(
            &[Resource {
                src,
                target: "app.exe".into()
            }],
            &[],
            "app",
            "windows"
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn resource_symlinks_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/", dir.path().join("escape")).unwrap();
        assert!(expand(
            &[Resource {
                src: dir.path().into(),
                target: "assets".into()
            }],
            &[],
            "app",
            "linux"
        )
        .is_err());
    }
}
