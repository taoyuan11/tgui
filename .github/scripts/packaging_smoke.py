#!/usr/bin/env python3
"""Install two versions of the packaging example, smoke-test, and remove them.

Runs only against artifacts explicitly passed on the command line. Each application
process has a timeout. Temporary applications, mounts and test data are cleaned in
finally blocks. Linux DEB tests require sudo on an isolated CI runner.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import uuid

from packaging_process import run_process

APP_ID = "com.tgui.packaging-example"


def run(args, *, cwd=None, env=None, timeout=90):
    print("+", " ".join(map(str, args)), flush=True)
    return run_process(args, cwd=cwd, env=env, timeout=timeout, capture_output=True)


def manifest(folder):
    data = json.loads((folder / "bundle-manifest.json").read_text())
    assert data["plan"]["config"]["identifier"] == APP_ID, "only the packaging example is supported"
    for item in data["artifacts"]:
        path = folder / item["path"]
        assert path.parent == folder and path.exists(), path
        if "sha256" in item:
            digest = hashlib.sha256()
            with path.open("rb") as source:
                for chunk in iter(lambda: source.read(65536), b""):
                    digest.update(chunk)
            assert digest.hexdigest() == item["sha256"], path
    return data


def artifact(folder, data, suffix):
    matches = [folder / a["path"] for a in data["artifacts"] if a["path"].endswith(suffix)]
    assert len(matches) == 1, (suffix, matches)
    return matches[0]


def clean_env(root):
    env = os.environ.copy()
    for key in list(env):
        if key.startswith(("DYLD_", "LD_", "FFMPEG", "VCPKG", "PKG_CONFIG", "CARGO_", "TGUI_")):
            env.pop(key)
    user = root / "user"
    user.mkdir(exist_ok=True)
    env.update(HOME=str(user), XDG_DATA_HOME=str(user / "data"),
               XDG_STATE_HOME=str(user / "state"), LOCALAPPDATA=str(user / "local"))
    if sys.platform == "win32":
        windir = os.environ["SystemRoot"]
        env["PATH"] = os.pathsep.join([str(Path(windir) / "System32"), windir])
    else:
        env["PATH"] = "/usr/bin:/bin:/usr/sbin:/sbin"
    return env


def marker(root):
    if sys.platform == "darwin":
        return root / "user/Library/Application Support" / APP_ID / "installation-smoke.txt"
    if sys.platform == "win32":
        return root / "user/local" / APP_ID / "installation-smoke.txt"
    return root / "user/data" / APP_ID / "installation-smoke.txt"


def smoke(executable, root, env):
    if sys.platform == "darwin":
        resources = executable.parent.parent / "Resources"
    elif sys.platform == "win32":
        resources = executable.parent
    else:
        resources = executable.parent.parent / "lib" / executable.name
    dependencies = json.loads((resources / "tgui-native-dependencies.json").read_text())
    assert "executable_imports" in dependencies and "libraries" in dependencies
    result = run([executable, "--smoke-test"], cwd=root, env=env, timeout=30)
    print(result.stdout, result.stderr, flush=True)
    assert marker(root).read_text() == "preserve across upgrade"


def macos(folders, records, root, env):
    installed = root / "Installed App.app"
    mount = root / "mount"
    mount.mkdir()
    for folder, record in zip(folders, records):
        dmg = artifact(folder, record, ".dmg")
        attached = False
        try:
            run(["hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", mount, dmg])
            attached = True
            assert (mount / "Applications").is_symlink()
            app = mount / (record["plan"]["config"]["product-name"] + ".app")
            if installed.exists():
                shutil.rmtree(installed)
            run(["ditto", app, installed])
        finally:
            if attached or os.path.ismount(mount):
                run(["hdiutil", "detach", mount])
        run(["codesign", "--verify", "--deep", "--strict", installed])
        smoke(installed / "Contents/MacOS/packaging", root, env)
    shutil.rmtree(installed)


def windows(folders, records, root, env):
    import winreg
    # Refuse to alter an existing installation owned by someone else.
    key = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\tgui Packaging"
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key):
            raise RuntimeError("packaging example is already installed")
    except FileNotFoundError:
        pass
    installed = root / "installed"
    try:
        for folder, record in zip(folders, records):
            run([artifact(folder, record, ".exe"), "/S", "/D=" + str(installed)], timeout=180)
            with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key) as handle:
                assert winreg.QueryValueEx(handle, "DisplayVersion")[0] == record["plan"]["version"]
            smoke(installed / "packaging.exe", root, env)
    finally:
        uninstaller = installed / "uninstall.exe"
        if uninstaller.exists():
            # _?= keeps NSIS in-place and waitable instead of launching a detached copy.
            run([uninstaller, "/S", "_?=" + str(installed)], timeout=180)
    assert not (installed / "packaging.exe").exists()
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, key):
            raise AssertionError("uninstall registry entry remains")
    except FileNotFoundError:
        pass


def linux(folders, records, root, env):
    existing = subprocess.run(["dpkg-query", "-W", "-f=${Status}", "packaging"], capture_output=True, text=True)
    assert "install ok installed" not in existing.stdout, "packaging is already installed"
    installed = False
    try:
        for folder, record in zip(folders, records):
            installed = True  # Also clean up partially configured packages on failure.
            run(["sudo", "apt-get", "install", "-y", artifact(folder, record, ".deb")], timeout=300)
            assert run(["dpkg-query", "-W", "-f=${Version}", "packaging"]).stdout == record["plan"]["version"]
            assert Path("/usr/share/applications/packaging.desktop").is_file()
            smoke(Path("/usr/bin/packaging"), root, env)
    finally:
        if installed:
            run(["sudo", "apt-get", "remove", "-y", "packaging"], timeout=180)
    assert not Path("/usr/bin/packaging").exists()

    # AppImage must work with no host FFmpeg search path, including extracted mode.
    # A clean container verifies that system FFmpeg installed for the DEB cannot
    # accidentally satisfy missing AppImage dependencies.
    run(["docker", "pull", "ubuntu:24.04"], timeout=300)
    for index, (folder, record) in enumerate(zip(folders, records)):
        image = artifact(folder, record, ".AppImage")
        image.chmod(image.stat().st_mode | 0o111)
        portable = root / "application.AppImage"
        shutil.copy2(image, portable)
        run([portable, "--appimage-extract-and-run", "--smoke-test"], cwd=root, env=env, timeout=60)
        extract = root / ("extracted-" + str(index))
        extract.mkdir()
        run([portable, "--appimage-extract"], cwd=extract, env=env)
        appdir = extract / "squashfs-root"
        assert (appdir / "usr/lib/packaging/tgui-native-dependencies.json").is_file()
        for library in record["native-dependencies"]["libraries"]:
            assert (appdir / "usr/lib" / library["name"]).exists(), library["name"]
        run([appdir / "AppRun", "--smoke-test"], cwd=root, env=env, timeout=30)
        container = "tgui-appimage-smoke-" + uuid.uuid4().hex
        try:
            run(["docker", "run", "--rm", "--name", container, "--network", "none", "--read-only",
                 "--tmpfs", "/tmp", "-e", "HOME=/tmp", "-v", str(appdir) + ":/app:ro",
                 "ubuntu:24.04", "/app/AppRun", "--smoke-test"], timeout=60)
        finally:
            subprocess.run(["docker", "rm", "-f", container], capture_output=True, timeout=30, check=False)
        portable.unlink()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("initial", type=Path)
    parser.add_argument("upgrade", type=Path)
    options = parser.parse_args()
    folders = [options.initial.resolve(), options.upgrade.resolve()]
    records = [manifest(folder) for folder in folders]
    assert records[0]["plan"]["version"] != records[1]["plan"]["version"], "need two distinct versions"
    with tempfile.TemporaryDirectory(prefix="tgui-install-test-") as temporary:
        root = Path(temporary)
        env = clean_env(root)
        handler = {"darwin": macos, "win32": windows, "linux": linux}[sys.platform]
        handler(folders, records, root, env)
        assert marker(root).read_text() == "preserve across upgrade", "uninstall removed user data"
    print("Install, launch, upgrade, uninstall and data-preservation checks passed.")


if __name__ == "__main__":
    main()
