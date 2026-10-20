//! Locations for installed application resources and per-user writable files.

use std::io;
use std::path::{Component, Path, PathBuf};

/// Paths shared by applications packaged with `cargo tgui bundle`.
///
/// Constructing this value does not create directories. Use [`Self::with_resource_dir`]
/// explicitly for development; installed applications never search the source tree
/// or the process working directory. Keep the identifier equal to the bundle identifier.
#[derive(Clone, Debug)]
pub struct AppPaths {
    resource_dir: PathBuf,
    data_dir: PathBuf,
    log_dir: PathBuf,
}

impl AppPaths {
    /// Resolve paths using the executable and the current user's platform directories.
    pub fn new(identifier: &str) -> io::Result<Self> {
        validate_identifier(identifier)?;
        let executable = std::env::current_exe()?;
        let resource_dir = resource_root(&executable, std::env::consts::OS)?;
        let (data_dir, log_dir) = user_roots(identifier)?;
        Ok(Self {
            resource_dir,
            data_dir,
            log_dir,
        })
    }

    /// Override the resource root, for example with an absolute development assets directory.
    /// Relative paths are rejected so changing the working directory cannot change resolution.
    pub fn with_resource_dir(mut self, directory: impl Into<PathBuf>) -> io::Result<Self> {
        let directory = directory.into();
        if !directory.is_absolute() {
            return Err(invalid("resource directory must be absolute"));
        }
        self.resource_dir = directory;
        Ok(self)
    }

    /// Directory containing packaged resources; it may not exist if no resources were packaged.
    pub fn resource_dir(&self) -> &Path {
        &self.resource_dir
    }

    /// Resolve a relative resource name. Absolute paths and parent traversal are rejected.
    pub fn resource(&self, relative: impl AsRef<Path>) -> io::Result<PathBuf> {
        let relative = relative.as_ref();
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
        {
            return Err(invalid(
                "resource name must be relative and cannot contain '..'",
            ));
        }
        Ok(self.resource_dir.join(relative))
    }

    /// Per-user persistent application data. Installers preserve this directory.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Per-user log directory, suitable for `LogFileConfig::log_dir`.
    pub fn log_dir(&self) -> &Path {
        &self.log_dir
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn validate_identifier(identifier: &str) -> io::Result<()> {
    if identifier.split('.').count() < 2
        || identifier.split('.').any(|part| {
            part.is_empty() || !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return Err(invalid(
            "application identifier must be a reverse-domain identifier",
        ));
    }
    Ok(())
}

fn resource_root(executable: &Path, os: &str) -> io::Result<PathBuf> {
    let parent = executable
        .parent()
        .ok_or_else(|| invalid("executable has no parent"))?;
    match os {
        "macos" => Ok(parent.join("../Resources")),
        "windows" => Ok(parent.to_path_buf()),
        // Both /usr/bin/app and <AppDir>/usr/bin/app have resources in ../lib/app.
        // This also works for --appimage-extract-and-run without trusting APPDIR.
        "linux" => Ok(parent.join("../lib").join(
            executable
                .file_name()
                .ok_or_else(|| invalid("executable has no name"))?,
        )),
        _ => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported desktop platform",
        )),
    }
}

fn absolute_env(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

fn user_roots(identifier: &str) -> io::Result<(PathBuf, PathBuf)> {
    #[cfg(target_os = "windows")]
    {
        let base = absolute_env("LOCALAPPDATA")
            .ok_or_else(|| invalid("LOCALAPPDATA must be an absolute directory"))?;
        let data = base.join(identifier);
        Ok((data.clone(), data.join("logs")))
    }
    #[cfg(target_os = "macos")]
    {
        let home =
            absolute_env("HOME").ok_or_else(|| invalid("HOME must be an absolute directory"))?;
        Ok((
            home.join("Library/Application Support").join(identifier),
            home.join("Library/Logs").join(identifier),
        ))
    }
    #[cfg(target_os = "linux")]
    {
        let home = absolute_env("HOME");
        let data = absolute_env("XDG_DATA_HOME")
            .or_else(|| home.as_ref().map(|p| p.join(".local/share")))
            .ok_or_else(|| invalid("XDG_DATA_HOME or HOME must be an absolute directory"))?;
        let state = absolute_env("XDG_STATE_HOME")
            .or_else(|| home.as_ref().map(|p| p.join(".local/state")))
            .ok_or_else(|| invalid("XDG_STATE_HOME or HOME must be an absolute directory"))?;
        Ok((data.join(identifier), state.join(identifier).join("logs")))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        let _ = identifier;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "unsupported desktop platform",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_resource_layouts_do_not_depend_on_working_directory() {
        for (os, exe, expected) in [
            ("windows", "/install/My App/app.exe", "/install/My App"),
            (
                "macos",
                "/Applications/My App.app/Contents/MacOS/app",
                "/Applications/My App.app/Contents/MacOS/../Resources",
            ),
            ("linux", "/usr/bin/app", "/usr/bin/../lib/app"),
            (
                "linux",
                "/tmp/.mount_example/usr/bin/app",
                "/tmp/.mount_example/usr/bin/../lib/app",
            ),
            (
                "linux",
                "/tmp/解包/squashfs-root/usr/bin/app",
                "/tmp/解包/squashfs-root/usr/bin/../lib/app",
            ),
        ] {
            assert_eq!(
                resource_root(Path::new(exe), os).unwrap(),
                Path::new(expected)
            );
        }
    }

    #[test]
    fn resource_names_and_development_overrides_are_checked() {
        let root = std::env::temp_dir().join("tgui path test");
        let paths = AppPaths {
            resource_dir: root.clone(),
            data_dir: root.clone(),
            log_dir: root.clone(),
        };
        assert_eq!(
            paths.resource("图片/logo.png").unwrap(),
            root.join("图片/logo.png")
        );
        for name in ["", "../secret", "assets/../../secret"] {
            assert!(paths.resource(name).is_err());
        }
        assert!(paths.resource(&root).is_err());
        assert!(paths.clone().with_resource_dir("relative").is_err());
        assert_eq!(paths.with_resource_dir(&root).unwrap().resource_dir(), root);
    }

    #[test]
    fn identifiers_cannot_escape_user_directories() {
        for id in [
            "",
            "..",
            "../app",
            "app",
            "com..app",
            "com.app/name",
            "com.app\\name",
        ] {
            assert!(validate_identifier(id).is_err());
        }
        assert!(validate_identifier("com.example.my-app").is_ok());
    }
}
