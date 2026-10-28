#!/usr/bin/env python3
"""Build two example versions without leaving manifest/lockfile edits behind."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

from packaging_process import run_process

ROOT = Path(__file__).resolve().parents[2]


def run(args):
    print("+", " ".join(map(str, args)), flush=True)
    run_process(args, cwd=ROOT, timeout=2400)


def notices(destination):
    """Preserve installed package notices alongside CI media example resources."""
    destination.mkdir(parents=True, exist_ok=True)
    sources = []
    if sys.platform == "darwin":
        packages = ["ffmpeg"] + subprocess.check_output(["brew", "deps", "--installed", "ffmpeg"], text=True).split()
        for package in packages:
            prefix = Path(subprocess.check_output(["brew", "--prefix", package], text=True).strip())
            for path in prefix.rglob("*"):
                if path.is_file() and (path.name.upper().startswith(("LICENSE", "COPYING")) or path.name == "copyright"):
                    sources.append((package, path))
    elif sys.platform == "win32":
        root = Path(os.environ["VCPKG_ROOT"]) / "installed/x64-windows/share"
        sources = [(p.parent.name, p) for p in root.glob("*/copyright")]
    else:
        # Include notices for the installed shared-library packages on the runner.
        sources = [(p.parent.name, p) for p in Path("/usr/share/doc").glob("*/copyright")]
    for index, (name, source) in enumerate(sources):
        shutil.copyfile(source, destination / (name.replace("/", "_") + "-" + str(index) + ".txt"))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--flavor", choices=["basic", "media"], required=True)
    options = parser.parse_args()
    manifest = ROOT / "examples/packaging/Cargo.toml"
    lock = ROOT / "Cargo.lock"
    original, original_lock = manifest.read_bytes(), lock.read_bytes()
    license_dir = manifest.parent / "assets/third-party"
    license_created = False
    binary = ROOT / ("target/debug/cargo-tgui.exe" if sys.platform == "win32" else "target/debug/cargo-tgui")
    output = ROOT / "target/packaging-ci"
    # Normalize checkout line endings before matching/inserting TOML lines.
    # Keep the original bytes above for exact restoration in the finally block.
    configured = original.decode("utf-8").replace("\r\n", "\n")
    if sys.platform == "win32" and options.flavor == "media":
        libs = str(Path(os.environ["VCPKG_ROOT"]) / "installed/x64-windows/bin")
        configured = configured.replace("[package.metadata.tgui.bundle]\n", "[package.metadata.tgui.bundle]\nlibrary-search-paths = [" + json.dumps(libs) + "]\n")
    try:
        if options.flavor == "media":
            if license_dir.exists():
                raise RuntimeError("refusing to overwrite existing assets/third-party")
            license_created = True
            notices(license_dir)
        for version in ["0.1.0", "0.1.1"]:
            # Binary output avoids platform newline translation (CRLF -> CRCRLF).
            manifest.write_bytes(configured.replace('version = "0.1.0"', 'version = "' + version + '"').encode("utf-8"))
            if version != "0.1.0":
                run(["cargo", "update", "-p", "packaging", "--offline"])
            command = [binary, "bundle", "--manifest-path", manifest, "--locked", "--out-dir", output / version]
            if options.flavor == "media":
                command.extend(["--features", "media"])
            run(command)
        run([sys.executable, ROOT / ".github/scripts/packaging_smoke.py", output / "0.1.0", output / "0.1.1"])
    finally:
        manifest.write_bytes(original)
        lock.write_bytes(original_lock)
        if license_created and license_dir.exists():
            shutil.rmtree(license_dir)


if __name__ == "__main__":
    main()
