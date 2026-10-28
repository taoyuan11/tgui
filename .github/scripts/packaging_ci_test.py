"""Regression checks for temporary manifests used by the packaging CI driver."""
from pathlib import Path
import sys
import tempfile
import tomllib
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import packaging_ci


def windows_write_text(path, data, encoding=None, errors=None, newline=None):
    """Exercise Windows text newline translation even on a Unix test host."""
    with path.open("w", encoding=encoding, errors=errors,
                   newline="\r\n" if newline is None else newline) as stream:
        return stream.write(data)


class PackagingCiTests(unittest.TestCase):
    def exercise(self, newline, flavor, fail_at=None):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest = root / "examples/packaging/Cargo.toml"
            manifest.parent.mkdir(parents=True)
            source = (packaging_ci.ROOT / "examples/packaging/Cargo.toml").read_text(encoding="utf-8")
            source += '\n# UTF-8 metadata: 打包示例\n'
            original = source.replace("\n", newline).encode("utf-8")
            original_lock = b'# Original lockfile\r\nversion = 4\r\n'
            manifest.write_bytes(original)
            lock = root / "Cargo.lock"
            lock.write_bytes(original_lock)
            licenses = manifest.parent / "assets/third-party"
            vcpkg = root / "SDK with spaces 打包"
            commands = []
            versions = []

            def run(command):
                commands.append([str(arg) for arg in command])
                data = manifest.read_bytes()
                self.assertNotIn(b"\r\r\n", data)
                parsed = tomllib.loads(data.decode("utf-8"))
                self.assertIn("打包示例", data.decode("utf-8"))
                bundle = parsed["package"]["metadata"]["tgui"]["bundle"]
                if flavor == "media":
                    self.assertEqual(bundle["library-search-paths"],
                                     [str(vcpkg / "installed/x64-windows/bin")])
                    self.assertTrue(licenses.is_dir())
                else:
                    self.assertNotIn("library-search-paths", bundle)
                if str(command[1]) == "bundle":
                    versions.append(parsed["package"]["version"])
                    self.assertEqual("--features" in command, flavor == "media")
                # Cargo may change the lock even when a later operation fails.
                lock.write_bytes(b"changed by cargo")
                if len(commands) == fail_at:
                    raise RuntimeError("injected build or smoke failure")

            with patch.object(packaging_ci, "ROOT", root), \
                 patch.object(packaging_ci, "sys", SimpleNamespace(platform="win32", executable=sys.executable)), \
                 patch.object(sys, "argv", ["packaging_ci.py", "--flavor", flavor]), \
                 patch.dict("os.environ", {"VCPKG_ROOT": str(vcpkg)}), \
                 patch.object(Path, "write_text", windows_write_text), \
                 patch.object(packaging_ci, "notices", side_effect=lambda dest: dest.mkdir(parents=True)), \
                 patch.object(packaging_ci, "run", side_effect=run):
                if fail_at is None:
                    packaging_ci.main()
                else:
                    with self.assertRaisesRegex(RuntimeError, "injected"):
                        packaging_ci.main()

            self.assertEqual(manifest.read_bytes(), original)
            self.assertEqual(lock.read_bytes(), original_lock)
            self.assertFalse(licenses.exists())
            if fail_at is None:
                self.assertEqual(versions, ["0.1.0", "0.1.1"])
                self.assertEqual(commands[1], ["cargo", "update", "-p", "packaging", "--offline"])
                self.assertEqual(commands[-1], [sys.executable,
                    str(root / ".github/scripts/packaging_smoke.py"),
                    str(root / "target/packaging-ci/0.1.0"),
                    str(root / "target/packaging-ci/0.1.1")])

    def test_lf_and_crlf_manifests_for_both_flavors(self):
        for newline in ["\n", "\r\n"]:
            for flavor in ["basic", "media"]:
                with self.subTest(newline=repr(newline), flavor=flavor):
                    self.exercise(newline, flavor)

    def test_original_bytes_restored_after_each_failed_command(self):
        for newline in ["\n", "\r\n"]:
            for flavor in ["basic", "media"]:
                for fail_at in range(1, 5):
                    with self.subTest(newline=repr(newline), flavor=flavor, fail_at=fail_at):
                        self.exercise(newline, flavor, fail_at)


if __name__ == "__main__":
    unittest.main()
