"""Tests of tools/runtime_closure.py on GNU ld 2.29.1 link maps.

    python3 -m unittest discover -s tests -p '*_test.py'
"""

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import runtime_closure as rc  # noqa: E402

# The head of a real map (examples/ui linked by symdev with GCCE's ld 2.29.1), paths
# shortened. `-L<gcc_lib>/` gives libgcc.a its double slash.
MAP = """\
Archive member included to satisfy reference by file (symbol)

/sdk/epoc32/release/armv5/urel/eexe.lib(uc_exe_.o)
                              (_E32Startup)
/sdk/epoc32/release/armv5/urel/eexe.lib(uc_exe.o)
                              /sdk/epoc32/release/armv5/urel/eexe.lib(uc_exe_.o) (RunThread)
/p/build/shims/libsymrs.a(symrs_avkon.o)
                              /p/build/cargo/libuidemo.a(uidemo.cgu.0.rcgu.o) (symrs_gc_clear)
/gcce/arm-none-symbianelf/lib/libsupc++.a(del_ops.o)
                              /p/build/shims/libsymrs.a(symrs_avkon.o) (operator delete(void*, unsigned int))
/gcce/arm-none-symbianelf/lib/libsupc++.a(eh_personality.o)
                              /p/build/shims/libsymrs.a(symrs_avkon.o) (__gxx_personality_v0)
/gcce/lib/gcc/arm-none-symbianelf/12.1.0//libgcc.a(pr-support.o)
                              /gcce/arm-none-symbianelf/lib/libsupc++.a(eh_personality.o) (__gnu_unwind_frame)
libgcc.a(_thumb1_case_uqi.o)  symrs_note.o (__gnu_thumb1_case_uqi)

As-needed library included to satisfy reference by file (symbol)

efsrv{000a0000}.dso           /p/libuidemo.a(x.o) (RFs::Connect(int)@@efsrv{000a0000}[100039e4].dll)

Discarded input sections
"""

SHIPPED = ["libsupc++.a(del_ops.o)", "libsupc++.a(eh_personality.o)",
           "libgcc.a(pr-support.o)", "libgcc.a(_thumb1_case_uqi.o)"]


class ParseTest(unittest.TestCase):
    def test_reads_every_member_with_who_wanted_it_and_for_what(self):
        got = rc.included_members(MAP)
        self.assertEqual([(i.archive, i.member) for i in got], [
            ("eexe.lib", "uc_exe_.o"), ("eexe.lib", "uc_exe.o"),
            ("libsymrs.a", "symrs_avkon.o"), ("libsupc++.a", "del_ops.o"),
            ("libsupc++.a", "eh_personality.o"), ("libgcc.a", "pr-support.o"),
            ("libgcc.a", "_thumb1_case_uqi.o")])
        self.assertEqual(got[0].referrer, None)  # -u on the command line
        self.assertEqual(got[0].symbol, "_E32Startup")
        self.assertEqual(got[3].referrer, "/p/build/shims/libsymrs.a(symrs_avkon.o)")
        self.assertEqual(got[3].symbol, "operator delete(void*, unsigned int)")
        self.assertEqual(got[5].path, "/gcce/lib/gcc/arm-none-symbianelf/12.1.0//libgcc.a")

    def test_a_member_and_its_reference_on_one_line(self):
        last = rc.included_members(MAP)[-1]
        self.assertEqual((last.referrer, last.symbol), ("symrs_note.o", "__gnu_thumb1_case_uqi"))

    def test_the_section_ends_at_the_first_blank_line(self):
        members = [i.member for i in rc.included_members(MAP)]
        self.assertNotIn("efsrv{000a0000}.dso", " ".join(members))

    def test_a_map_without_the_section_is_an_error(self):
        # ld writes the section only when it took a member; a closure link always takes
        # some, so a map without it is the wrong file or a broken link.
        with self.assertRaisesRegex(rc.ClosureError, "no .Archive member included"):
            rc.included_members("Memory Configuration\n")

    def test_a_reference_line_without_a_member_line_is_an_error(self):
        text = MAP.replace("/p/build/shims/libsymrs.a(symrs_avkon.o)\n", "", 1)
        with self.assertRaisesRegex(rc.ClosureError, "line 7"):
            rc.included_members(text)


class CheckTest(unittest.TestCase):
    def test_the_shipped_set_is_exactly_what_the_link_pulled(self):
        self.assertEqual(rc.check(rc.included_members(MAP), SHIPPED), [])

    def test_a_member_the_link_needs_but_the_set_lacks(self):
        problems = rc.check(rc.included_members(MAP), SHIPPED[:-1])
        self.assertEqual(len(problems), 1)
        self.assertIn("libgcc.a(_thumb1_case_uqi.o) is needed", problems[0])
        self.assertIn("symrs_note.o (__gnu_thumb1_case_uqi)", problems[0])

    def test_a_shipped_member_nothing_needs(self):
        problems = rc.check(rc.included_members(MAP), SHIPPED + ["libgcc.a(_udivsi3.o)"])
        self.assertEqual(problems, ["libgcc.a(_udivsi3.o) is shipped but no shim needs it"])

    def test_only_the_archives_the_set_names_are_checked(self):
        # eexe.lib and libsymrs.a members are not runtime members.
        self.assertEqual(rc.check(rc.included_members(MAP), SHIPPED), [])

    def test_a_malformed_member_name_is_an_error(self):
        with self.assertRaisesRegex(rc.ClosureError, "libgcc.a:x.o"):
            rc.check([], ["libgcc.a:x.o"])


class MainTest(unittest.TestCase):
    def run_main(self, members):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp, "closure.map")
            path.write_text(MAP)
            out, err = [], []
            code = rc.main([str(path)] + members, out=out.append, err=err.append)
            return code, out, err

    def test_success_lists_each_member_and_why(self):
        code, out, err = self.run_main(SHIPPED)
        self.assertEqual(code, 0)
        self.assertEqual(err, [])
        self.assertIn("libgcc.a(pr-support.o) <- libsupc++.a(eh_personality.o) "
                      "(__gnu_unwind_frame)", out)

    def test_a_mismatch_fails(self):
        code, _, err = self.run_main(SHIPPED[1:])
        self.assertEqual(code, 1)
        self.assertIn("libsupc++.a(del_ops.o) is needed", err[0])


if __name__ == "__main__":
    unittest.main()
