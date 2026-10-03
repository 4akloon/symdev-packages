"""Tests of tools/sdk_casefold.py on a fake epoc32/include tree.

    python3 -m unittest discover -s tests -p '*_test.py'
"""

import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import sdk_casefold as cf  # noqa: E402


def tree(root, files):
    for rel, text in files.items():
        path = Path(root, rel)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(text if isinstance(text, bytes) else text.encode())


def links(out):
    """{relative link: relative target} of every symlink under `out`."""
    found = {}
    for dirpath, _, names in os.walk(out):
        for name in names:
            path = Path(dirpath, name)
            if path.is_symlink():
                found[str(path.relative_to(out))] = os.readlink(path)
    return found


class OverlayTest(unittest.TestCase):
    def test_links_each_include_name_the_tree_has_only_in_another_case(self):
        with tempfile.TemporaryDirectory() as tmp:
            inc, out = Path(tmp, "include"), Path(tmp, "overlay")
            tree(inc, {
                "fbs.h": "#include <FbsMessage.h>\n#include <e32std.h>\n",
                "fbsmessage.h": "",
                "e32std.h": '  #  include "Variant\\Symbian_OS.hrh"\n',
                "variant/symbian_os.hrh": "",
            })
            cf.ensure(inc, out)
            self.assertEqual(links(out), {
                "FbsMessage.h": str(inc / "fbsmessage.h"),
                "Variant/Symbian_OS.hrh": str(inc / "variant/symbian_os.hrh"),
            })
            self.assertEqual((out / ".symdev-casefold").read_text(), str(inc))

    def test_names_that_exist_as_written_or_not_at_all_get_no_link(self):
        with tempfile.TemporaryDirectory() as tmp:
            inc, out = Path(tmp, "include"), Path(tmp, "overlay")
            tree(inc, {"a.h": "#include <e32std.h>\n#include <Missing.h>\n"
                              "#include </abs/Path.h>\n#include <../Up.h>\n"
                              "#include MACRO_NAME\n// #include <A.H>\n",
                       "e32std.h": "", "up.h": "", "path.h": ""})
            cf.ensure(inc, out)
            self.assertEqual(links(out), {})

    def test_a_name_two_files_share_up_to_case_takes_the_first_in_sorted_order(self):
        with tempfile.TemporaryDirectory() as tmp:
            inc, out = Path(tmp, "include"), Path(tmp, "overlay")
            tree(inc, {"x.h": "#include <DUP.h>\n", "Dup.h": "", "dup.h": ""})
            cf.ensure(inc, out)
            self.assertEqual(links(out), {"DUP.h": str(inc / "Dup.h")})

    def test_an_existing_overlay_is_reused(self):
        with tempfile.TemporaryDirectory() as tmp:
            inc, out = Path(tmp, "include"), Path(tmp, "overlay")
            tree(inc, {"x.h": "#include <Y.h>\n", "y.h": ""})
            cf.ensure(inc, out)
            (inc / "z.h").write_text("#include <W.h>\n")
            (inc / "w.h").write_text("")
            cf.ensure(inc, out)
            self.assertEqual(set(links(out)), {"Y.h"})

    def test_bytes_that_are_not_utf8_are_read_as_far_as_they_go(self):
        with tempfile.TemporaryDirectory() as tmp:
            inc, out = Path(tmp, "include"), Path(tmp, "overlay")
            tree(inc, {"x.h": b"// caf\xe9\n#include <Y.h>\n", "y.h": ""})
            cf.ensure(inc, out)
            self.assertEqual(set(links(out)), {"Y.h"})

    def test_a_missing_include_directory_is_an_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(cf.CaseFoldError, "not a directory"):
                cf.ensure(Path(tmp, "nope"), Path(tmp, "overlay"))


if __name__ == "__main__":
    unittest.main()
