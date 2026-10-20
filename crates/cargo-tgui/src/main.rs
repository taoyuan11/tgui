mod bundle;
mod config;
mod native;
mod resources;

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use serde::Serialize;

#[derive(Parser)]
#[command(
    name = "cargo tgui",
    version,
    about = "Build desktop bundles and installers"
)]
struct Cli {
    #[command(subcommand)]
    command: Action,
}

#[derive(Subcommand)]
enum Action {
    /// Build, collect runtime dependencies, and package an application.
    Bundle {
        #[command(flatten)]
        options: Options,
        /// Print the resolved plan without building, staging or downloading packaging tools.
        #[arg(long)]
        dry_run: bool,
    },
    /// Validate configuration and check the native packaging toolchain.
    Doctor(Options),
}

#[derive(Args, Clone, Debug, Serialize)]
pub struct Options {
    #[arg(long, default_value = "Cargo.toml")]
    manifest_path: PathBuf,
    #[arg(short = 'p', long)]
    package: Option<String>,
    #[arg(long)]
    bin: Option<String>,
    #[arg(long)]
    target: Option<String>,
    #[arg(long, default_value = "release")]
    profile: String,
    #[arg(long, value_delimiter = ',')]
    features: Vec<String>,
    #[arg(long)]
    all_features: bool,
    #[arg(long)]
    no_default_features: bool,
    #[arg(long)]
    locked: bool,
    #[arg(long = "format", value_delimiter = ',')]
    formats: Vec<config::Format>,
    #[arg(long)]
    out_dir: Option<PathBuf>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_some_and(|arg| arg == "tgui") {
        args.remove(1);
    }
    let cli = Cli::parse_from(args);
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    match cli.command {
        Action::Bundle { options, dry_run } => {
            let plan = config::Plan::resolve(options)?;
            if dry_run {
                println!("{}", serde_json::to_string_pretty(&plan)?);
                return Ok(());
            }
            doctor(&plan)?;
            bundle::run(&plan)
        }
        Action::Doctor(options) => doctor(&config::Plan::resolve(options)?),
    }
}

pub fn output(command: &mut std::process::Command) -> Result<String> {
    let description = format!("{command:?}");
    let result = command
        .output()
        .with_context(|| format!("could not run {description}"))?;
    if !result.status.success() {
        bail!(
            "{description} failed ({}):\n{}\n{}",
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    String::from_utf8(result.stdout).with_context(|| format!("non-UTF-8 output from {description}"))
}

fn doctor(plan: &config::Plan) -> Result<()> {
    use std::process::Command;
    output(Command::new("cargo").arg("--version"))?;
    for (tool, args) in match plan.os.as_str() {
        "macos" => vec![
            ("xcrun", vec!["--find", "install_name_tool"]),
            ("xcrun", vec!["--find", "codesign"]),
            ("hdiutil", vec!["help"]),
        ],
        "linux" => {
            let mut tools = vec![("ldd", vec!["--version"])];
            if plan.formats.contains(&config::Format::Deb) {
                tools.push(("dpkg-shlibdeps", vec!["--version"]));
            }
            if plan.formats.contains(&config::Format::Appimage) {
                tools.extend([
                    ("patchelf", vec!["--version"]),
                    ("mksquashfs", vec!["-version"]),
                ]);
            }
            tools
        }
        _ => vec![],
    } {
        output(Command::new(tool).args(args)).with_context(|| {
            format!("missing packaging prerequisite: {tool}; see docs/guide/packaging.md")
        })?;
    }
    eprintln!(
        "Ready: {} {} / {} / {:?}",
        plan.package_name, plan.version, plan.target, plan.formats
    );
    eprintln!("The backend may download NSIS/AppImage/DMG tools on first use. Signing credentials are not used.");
    Ok(())
}
