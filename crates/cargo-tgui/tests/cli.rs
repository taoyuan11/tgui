use std::{fs, path::Path, process::Command};

fn cli(manifest: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-tgui"))
        .args(["bundle", "--dry-run", "--manifest-path"])
        .arg(manifest)
        .args(args)
        .output()
        .unwrap()
}

fn app(root: &Path, name: &str, extra_bin: bool) {
    let base = root.join(name);
    fs::create_dir_all(base.join("src")).unwrap();
    fs::write(base.join("src/main.rs"), "fn main() {}\n").unwrap();
    if extra_bin {
        fs::write(base.join("src/other.rs"), "fn main() {}\n").unwrap();
    }
    fs::write(
        base.join("icon.png"),
        "fixture icon (not decoded in dry-run)",
    )
    .unwrap();
    fs::write(base.join("资源.txt"), "hello").unwrap();
    let extra = if extra_bin {
        "\n[[bin]]\nname = 'other'\npath = 'src/other.rs'\n"
    } else {
        ""
    };
    fs::write(
        base.join("Cargo.toml"),
        format!(
            r#"
[package]
name = "{name}"
version.workspace = true
edition = "2021"
{extra}
[package.metadata.tgui.bundle]
identifier = "com.example.{name}"
product-name = "Example App"
publisher = "Example"
icons = ["icon.png"]
resources = [{{ src = "资源.txt", target = "assets/资源.txt" }}]
"#
        ),
    )
    .unwrap();
}

#[test]
fn workspace_selection_inheritance_and_dry_run_are_non_mutating() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("含 空格");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = ['first', 'second']\nresolver = '2'\n[workspace.package]\nversion = '1.2.3'\n").unwrap();
    app(&root, "first", false);
    app(&root, "second", true);

    let ambiguous = cli(&root.join("Cargo.toml"), &[]);
    assert!(!ambiguous.status.success());
    assert!(String::from_utf8_lossy(&ambiguous.stderr).contains("workspace is ambiguous"));
    let selected = cli(&root.join("Cargo.toml"), &["-p", "first"]);
    assert!(
        selected.status.success(),
        "{}",
        String::from_utf8_lossy(&selected.stderr)
    );
    let plan: serde_json::Value = serde_json::from_slice(&selected.stdout).unwrap();
    assert_eq!(plan["version"], "1.2.3");
    assert_eq!(plan["binary"], "first");
    assert!(!plan["steps"].as_array().unwrap().is_empty());
    assert!(!root.join("target").exists());
    assert!(!root.join("Cargo.lock").exists());

    let target = root.join("custom 构建 directory");
    let custom = Command::new(env!("CARGO_BIN_EXE_cargo-tgui"))
        .args(["tgui", "bundle", "--dry-run", "--manifest-path"])
        .arg(root.join("first/Cargo.toml"))
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .unwrap();
    assert!(
        custom.status.success(),
        "{}",
        String::from_utf8_lossy(&custom.stderr)
    );
    let plan: serde_json::Value = serde_json::from_slice(&custom.stdout).unwrap();
    assert!(Path::new(plan["out_dir"].as_str().unwrap()).starts_with(target.join("tgui-dist")));
    assert!(!target.exists());

    let ambiguous_binary = cli(&root.join("second/Cargo.toml"), &[]);
    assert!(!ambiguous_binary.status.success());
    let explicit_binary = cli(&root.join("second/Cargo.toml"), &["--bin", "other"]);
    assert!(
        explicit_binary.status.success(),
        "{}",
        String::from_utf8_lossy(&explicit_binary.stderr)
    );
}

#[test]
fn invalid_formats_and_targets_fail_before_building() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[workspace]\nmembers = ['first']\n[workspace.package]\nversion = '1.0.0'\n",
    )
    .unwrap();
    app(temp.path(), "first", false);
    let manifest = temp.path().join("first/Cargo.toml");
    let invalid = cli(&manifest, &["--target", "unsupported-platform"]);
    assert!(!invalid.status.success());
    let invalid_format = if cfg!(windows) { "dmg" } else { "nsis" };
    assert!(!cli(&manifest, &["--format", invalid_format])
        .status
        .success());
    assert!(!temp.path().join("target").exists());
}
