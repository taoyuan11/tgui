use std::path::{Path, PathBuf};

use anyhow::{bail, ensure, Context, Result};
use cargo_metadata::{Metadata, MetadataCommand, Package};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::{resources, Options};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Nsis,
    App,
    Dmg,
    Deb,
    Appimage,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Resource {
    pub src: PathBuf,
    pub target: PathBuf,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BundleConfig {
    pub identifier: String,
    pub product_name: String,
    pub publisher: String,
    pub icons: Vec<PathBuf>,
    #[serde(default)]
    pub resources: Vec<Resource>,
    #[serde(default)]
    pub license_files: Vec<PathBuf>,
    #[serde(default)]
    pub library_search_paths: Vec<PathBuf>,
    #[serde(default)]
    pub windows: Windows,
    #[serde(default)]
    pub macos: Macos,
    #[serde(default)]
    pub linux: Linux,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Windows {
    pub install_mode: InstallMode,
    pub languages: Vec<String>,
}
impl Default for Windows {
    fn default() -> Self {
        Self {
            install_mode: InstallMode::CurrentUser,
            languages: vec!["English".into(), "SimpChinese".into()],
        }
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallMode {
    #[default]
    CurrentUser,
    PerMachine,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Macos {
    pub minimum_system_version: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, default)]
pub struct Linux {
    pub depends: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Plan {
    pub package_name: String,
    pub package_id: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub manifest: PathBuf,
    pub binary: String,
    pub target: String,
    pub os: String,
    pub out_dir: PathBuf,
    pub formats: Vec<Format>,
    pub config: BundleConfig,
    pub resource_files: Vec<resources::File>,
    pub build_args: Vec<String>,
    pub steps: Vec<String>,
    pub options: Options,
}

impl Plan {
    pub fn resolve(options: Options) -> Result<Self> {
        ensure!(
            !options.profile.is_empty()
                && options
                    .profile
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "invalid Cargo profile name"
        );
        let host = crate::output(std::process::Command::new("rustc").arg("-vV"))?
            .lines()
            .find_map(|l| l.strip_prefix("host: "))
            .context("rustc did not report its host target")?
            .to_owned();
        let target = options.target.clone().unwrap_or_else(|| host.clone());
        ensure!(
            target == host,
            "only native builds are supported: requested {target}, host {host}"
        );
        let os = match target.as_str() {
            "x86_64-pc-windows-msvc" => "windows",
            "aarch64-apple-darwin" | "x86_64-apple-darwin" => "macos",
            "x86_64-unknown-linux-gnu" => "linux",
            _ => bail!("unsupported target {target}"),
        }
        .to_owned();
        let manifest = options
            .manifest_path
            .canonicalize()
            .context("cannot find application Cargo.toml")?;
        let mut command = MetadataCommand::new();
        command.manifest_path(&manifest).no_deps();
        if options.locked {
            command.other_options(vec!["--locked".into()]);
        }
        let metadata = command.exec().context("cargo metadata failed")?;
        let package = select_package(&metadata, &manifest, options.package.as_deref())?;
        let binary = select_binary(package, options.bin.as_deref())?;
        let manifest = package.manifest_path.clone().into_std_path_buf();
        let base = manifest.parent().context("manifest has no parent")?;
        let value = package
            .metadata
            .get("tgui")
            .and_then(|v| v.get("bundle"))
            .context("missing [package.metadata.tgui.bundle]; see docs/guide/packaging.md")?;
        let mut config: BundleConfig =
            serde_json::from_value(value.clone()).context("invalid tgui bundle configuration")?;
        validate(&config)?;
        for path in config
            .icons
            .iter_mut()
            .chain(config.license_files.iter_mut())
            .chain(config.library_search_paths.iter_mut())
        {
            *path = dunce::canonicalize(base.join(&*path))
                .with_context(|| format!("missing configured path {}", path.display()))?;
        }
        ensure!(
            config.icons.iter().all(|p| p.is_file()),
            "icons must be files"
        );
        ensure!(
            config.license_files.iter().all(|p| p.is_file()),
            "license-files must be files"
        );
        ensure!(
            config.library_search_paths.iter().all(|p| p.is_dir()),
            "library-search-paths must be directories"
        );
        for resource in &mut config.resources {
            resource.src = base.join(&resource.src);
        }
        let resource_files =
            resources::expand(&config.resources, &config.license_files, &binary, &os)?;
        let mut formats = if options.formats.is_empty() {
            defaults(&os)
        } else {
            options.formats.clone()
        };
        formats.sort();
        formats.dedup();
        ensure!(
            formats.iter().all(|f| defaults(&os).contains(f)),
            "requested format is not supported on {os}"
        );
        let out_dir = options
            .out_dir
            .as_ref()
            .map(|p| base.join(p))
            .unwrap_or_else(|| {
                metadata
                    .target_directory
                    .as_std_path()
                    .join("tgui-dist")
                    .join(&package.name)
                    .join(&target)
                    .join(&options.profile)
            });
        let build_args = build_args(&options, &manifest, &package.name, &binary, &target);
        let steps = vec![
            "Check native toolchain and packaging prerequisites".into(),
            "Build the selected binary and locate it using Cargo JSON messages".into(),
            "Stage resources and recursively inspect native dependencies and architectures".into(),
            match os.as_str() {
                "macos" => "Relocate staged dylibs, create APP, ad-hoc sign, and create requested formats",
                "windows" => "Stage runtime DLLs and create the NSIS installer",
                _ => "Resolve DEB package dependencies and bundle AppImage libraries for requested formats",
            }.into(),
            "Write artifact manifest and SHA-256 checksums; move completed outputs into out_dir".into(),
        ];
        Ok(Self {
            package_name: package.name.clone(),
            package_id: package.id.to_string(),
            version: package.version.to_string(),
            description: package
                .description
                .clone()
                .unwrap_or_else(|| config.product_name.clone()),
            authors: package.authors.clone(),
            manifest,
            binary,
            target,
            os,
            out_dir,
            formats,
            config,
            resource_files,
            build_args,
            steps,
            options,
        })
    }
}

fn defaults(os: &str) -> Vec<Format> {
    match os {
        "windows" => vec![Format::Nsis],
        "macos" => vec![Format::App, Format::Dmg],
        _ => vec![Format::Deb, Format::Appimage],
    }
}

fn validate(config: &BundleConfig) -> Result<()> {
    ensure!(config.identifier.contains('.') && config.identifier.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')), "identifier must be a reverse-domain name");
    for (name, value) in [
        ("product-name", &config.product_name),
        ("publisher", &config.publisher),
    ] {
        ensure!(
            !value.trim().is_empty()
                && !value
                    .chars()
                    .any(|c| c.is_control() || "<>:\"/\\|?*$".contains(c))
                && !value.ends_with(['.', ' ']),
            "invalid {name}"
        );
    }
    ensure!(
        !config.icons.is_empty(),
        "at least one PNG/ICO/ICNS icon is required"
    );
    ensure!(
        !config.windows.languages.is_empty(),
        "windows.languages cannot be empty"
    );
    Ok(())
}

fn select_package<'a>(
    metadata: &'a Metadata,
    manifest: &Path,
    name: Option<&str>,
) -> Result<&'a Package> {
    let members: Vec<_> = metadata
        .packages
        .iter()
        .filter(|p| metadata.workspace_members.contains(&p.id))
        .collect();
    if let Some(name) = name {
        return members
            .into_iter()
            .find(|p| p.name == name)
            .with_context(|| format!("workspace package {name} not found"));
    }
    if let Some(package) = members
        .iter()
        .find(|p| p.manifest_path.as_std_path().canonicalize().ok().as_deref() == Some(manifest))
    {
        return Ok(package);
    }
    ensure!(
        members.len() == 1,
        "workspace is ambiguous; choose an application with -p"
    );
    members
        .first()
        .copied()
        .context("workspace has no packages")
}

fn select_binary(package: &Package, name: Option<&str>) -> Result<String> {
    let binaries: Vec<_> = package
        .targets
        .iter()
        .filter(|t| t.kind.iter().any(|k| k == "bin"))
        .collect();
    if let Some(name) = name {
        return binaries
            .iter()
            .find(|t| t.name == name)
            .map(|t| t.name.clone())
            .with_context(|| format!("binary {name} not found in {}", package.name));
    }
    ensure!(
        binaries.len() == 1,
        "package {} has {} binaries; choose an application and --bin explicitly",
        package.name,
        binaries.len()
    );
    Ok(binaries[0].name.clone())
}

fn build_args(
    options: &Options,
    manifest: &Path,
    package: &str,
    binary: &str,
    target: &str,
) -> Vec<String> {
    let mut args = vec![
        "build".into(),
        "--manifest-path".into(),
        manifest.to_string_lossy().into_owned(),
        "--package".into(),
        package.into(),
        "--bin".into(),
        binary.into(),
        "--target".into(),
        target.into(),
        "--profile".into(),
        options.profile.clone(),
        "--message-format=json-render-diagnostics".into(),
    ];
    if options.locked {
        args.push("--locked".into());
    }
    if options.no_default_features {
        args.push("--no-default-features".into());
    }
    if options.all_features {
        args.push("--all-features".into());
    }
    if !options.features.is_empty() {
        args.extend(["--features".into(), options.features.join(",")]);
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn build_arguments_preserve_paths_and_feature_flags() {
        let crate::Action::Bundle { options, .. } = crate::Cli::parse_from([
            "cargo-tgui",
            "bundle",
            "--features",
            "audio,video",
            "--no-default-features",
            "--all-features",
            "--locked",
            "--profile",
            "dev",
        ])
        .command
        else {
            panic!()
        };
        let args = build_args(
            &options,
            Path::new("/含 空格/Cargo.toml"),
            "demo",
            "gui",
            "native",
        );
        assert_eq!(args[2], "/含 空格/Cargo.toml");
        assert!(args.contains(&"--locked".into()));
        assert!(args.contains(&"--no-default-features".into()));
        assert!(args.contains(&"--all-features".into()));
        assert_eq!(args.last().unwrap(), "audio,video");
    }
}
