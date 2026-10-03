"""Tests of tools/sdk_free.py on a fake SDK tree and fake build outputs.

    python3 -m unittest discover -s tests -p '*_test.py'
"""

import contextlib
import gzip
import io
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import sdk_free  # noqa: E402

HEADER = b"// e32std.h, a file of the SDK\nclass TDesC;\n"


def tree(root, files):
    for rel, data in files.items():
        path = Path(root, rel)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)


def tar_bytes(files, gz=False):
    """A tar (gzipped if `gz`) of {name: bytes}."""
    raw = io.BytesIO()
    with tarfile.open(fileobj=raw, mode="w") as tar:
        for name, data in files.items():
            info = tarfile.TarInfo(name)
            info.size = len(data)
            tar.addfile(info, io.BytesIO(data))
    data = raw.getvalue()
    return gzip.compress(data, mtime=0) if gz else data


class SdkFreeTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.sdk = self.root / "sdk"
        tree(self.sdk, {
            "epoc32/include/e32std.h": HEADER,
            "epoc32/include/empty.h": b"",
        })

    def tearDown(self):
        self.tmp.cleanup()

    def check(self, *paths):
        return sdk_free.SdkFiles.of(self.sdk).leaks(paths)

    def test_outputs_without_sdk_content_pass(self):
        out = self.root / "out"
        tree(out, {
            "rust-sdk/symbian-rs/prebuilt/lib/libsymrs.a": b"!<arch>\nobjects",
            "rust-sdk/symbian-rs/shims/s60/symrs_avkon.cpp": b"// MIT\n",
        })
        self.assertEqual(self.check(out), [])

    def test_an_sdk_file_under_another_name_in_a_nested_archive_is_found(self):
        inner = tar_bytes({"a/renamed.txt": HEADER}, gz=True)
        outer = self.root / "symdev-out.tar"
        outer.write_bytes(tar_bytes({"out/x-source.tar.gz": inner, "out/ok": b"fine"}))
        leaks = self.check(outer)
        self.assertEqual(len(leaks), 1)
        self.assertIn("out/x-source.tar.gz", leaks[0])
        self.assertIn("a/renamed.txt", leaks[0])
        self.assertIn("epoc32/include/e32std.h", leaks[0])

    def test_a_path_through_epoc32_is_found_whatever_its_case(self):
        packed = self.root / "packed.tar.gz"
        packed.write_bytes(tar_bytes({"x/EPOC32/release/note.txt": b"not the SDK's"}, gz=True))
        leaks = self.check(packed)
        self.assertEqual(len(leaks), 1)
        self.assertIn("x/EPOC32/release/note.txt", leaks[0])

    def test_only_the_part_below_a_named_directory_is_checked_for_epoc32(self):
        named = self.root / "EPOC32" / "notes"
        tree(named, {"epoc32/x.txt": b"one", "y.txt": b"two"})
        leaks = self.check(named)
        self.assertEqual(len(leaks), 1)
        self.assertIn("epoc32/x.txt", leaks[0])

    def test_a_link_into_the_sdk_in_a_directory_is_found(self):
        out = self.root / "out"
        out.mkdir()
        (out / "header").symlink_to(self.sdk / "epoc32/include/e32std.h")
        self.assertEqual(len(self.check(out)), 1)

    def test_empty_files_match_nothing(self):
        out = self.root / "out"
        tree(out, {"empty": b""})
        self.assertEqual(self.check(out), [])

    def test_an_sdk_without_files_is_refused_so_the_check_cannot_pass_vacuously(self):
        empty = self.root / "nothing"
        empty.mkdir()
        with self.assertRaises(sdk_free.SdkFreeError):
            sdk_free.SdkFiles.of(empty)
        with self.assertRaises(sdk_free.SdkFreeError):
            sdk_free.SdkFiles.of(self.root / "missing")

    def test_a_path_that_does_not_exist_is_refused(self):
        with self.assertRaises(sdk_free.SdkFreeError):
            self.check(self.root / "missing.tar")

    def test_main_exits_1_on_a_leak_and_0_without(self):
        out = self.root / "out"
        tree(out, {"ok": b"fine"})
        argv = ["sdk_free.py", str(self.sdk), str(out)]
        said = io.StringIO()
        with contextlib.redirect_stdout(said), contextlib.redirect_stderr(said):
            self.assertEqual(sdk_free.main(argv), 0)
            tree(out, {"copy.h": HEADER})
            self.assertEqual(sdk_free.main(argv), 1)
            self.assertEqual(sdk_free.main(["sdk_free.py", str(self.sdk)]), 2)
        self.assertIn("copy.h: the bytes of the SDK's epoc32/include/e32std.h", said.getvalue())


if __name__ == "__main__":
    unittest.main()
