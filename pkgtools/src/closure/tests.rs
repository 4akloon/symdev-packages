//! The Python tool's tests (tests/runtime_closure_test.py), case for case, on GNU ld 2.29.1
//! link maps.

use super::{ClosureTool, Inclusion, LinkMap, ShippedSet};

/// The head of a real map (examples/ui linked by symdev with GCCE's ld 2.29.1), paths
/// shortened. `-L<gcc_lib>/` gives libgcc.a its double slash.
const MAP: &str = "\
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
";

const SHIPPED: [&str; 4] = [
    "libsupc++.a(del_ops.o)",
    "libsupc++.a(eh_personality.o)",
    "libgcc.a(pr-support.o)",
    "libgcc.a(_thumb1_case_uqi.o)",
];

fn included(text: &str) -> Vec<Inclusion> {
    LinkMap::included_members(text).unwrap()
}

fn check(shipped: &[&str]) -> Vec<String> {
    ShippedSet::new(shipped).unwrap().check(&included(MAP))
}

#[test]
fn reads_every_member_with_who_wanted_it_and_for_what() {
    let got = included(MAP);
    let pairs: Vec<(&str, &str)> = got
        .iter()
        .map(|i| (i.archive(), i.member.as_str()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("eexe.lib", "uc_exe_.o"),
            ("eexe.lib", "uc_exe.o"),
            ("libsymrs.a", "symrs_avkon.o"),
            ("libsupc++.a", "del_ops.o"),
            ("libsupc++.a", "eh_personality.o"),
            ("libgcc.a", "pr-support.o"),
            ("libgcc.a", "_thumb1_case_uqi.o"),
        ]
    );
    assert_eq!(got[0].referrer, None); // -u on the command line
    assert_eq!(got[0].symbol, "_E32Startup");
    assert_eq!(
        got[3].referrer.as_deref(),
        Some("/p/build/shims/libsymrs.a(symrs_avkon.o)")
    );
    assert_eq!(got[3].symbol, "operator delete(void*, unsigned int)");
    assert_eq!(
        got[5].path,
        "/gcce/lib/gcc/arm-none-symbianelf/12.1.0//libgcc.a"
    );
}

#[test]
fn a_member_and_its_reference_on_one_line() {
    let got = included(MAP);
    let last = got.last().unwrap();
    assert_eq!(
        (last.referrer.as_deref(), last.symbol.as_str()),
        (Some("symrs_note.o"), "__gnu_thumb1_case_uqi")
    );
}

#[test]
fn the_section_ends_at_the_first_blank_line() {
    let members: Vec<String> = included(MAP).into_iter().map(|i| i.member).collect();
    assert!(!members.join(" ").contains("efsrv{000a0000}.dso"));
}

#[test]
fn a_map_without_the_section_is_an_error() {
    // ld writes the section only when it took a member; a closure link always takes some,
    // so a map without it is the wrong file or a broken link.
    let error = LinkMap::included_members("Memory Configuration\n").unwrap_err();
    assert!(
        error.to_string().contains("no 'Archive member included"),
        "{error}"
    );
}

#[test]
fn a_reference_line_without_a_member_line_is_an_error() {
    let text = MAP.replacen("/p/build/shims/libsymrs.a(symrs_avkon.o)\n", "", 1);
    let error = LinkMap::included_members(&text).unwrap_err();
    assert!(error.to_string().contains("line 7"), "{error}");
}

#[test]
fn the_shipped_set_is_exactly_what_the_link_pulled() {
    assert_eq!(check(&SHIPPED), Vec::<String>::new());
}

#[test]
fn a_member_the_link_needs_but_the_set_lacks() {
    let problems = check(&SHIPPED[..3]);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("libgcc.a(_thumb1_case_uqi.o) is needed"));
    assert!(problems[0].contains("symrs_note.o (__gnu_thumb1_case_uqi)"));
}

#[test]
fn a_shipped_member_nothing_needs() {
    let mut shipped = SHIPPED.to_vec();
    shipped.push("libgcc.a(_udivsi3.o)");
    assert_eq!(
        check(&shipped),
        ["libgcc.a(_udivsi3.o) is shipped but no shim needs it"]
    );
}

#[test]
fn only_the_archives_the_set_names_are_checked() {
    // eexe.lib and libsymrs.a members are not runtime members.
    assert_eq!(check(&SHIPPED), Vec::<String>::new());
}

#[test]
fn a_malformed_member_name_is_an_error() {
    let error = ShippedSet::new(&["libgcc.a:x.o"]).err().unwrap();
    assert!(error.to_string().contains("libgcc.a:x.o"), "{error}");
}

/// Runs the tool on a file holding [`MAP`]: (exit code, stdout lines, stderr lines).
fn run_main(members: &[&str]) -> (u8, Vec<String>, Vec<String>) {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("closure.map");
    std::fs::write(&path, MAP).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let shipped: Vec<String> = members.iter().map(|m| m.to_string()).collect();
    let code = ClosureTool::run(&path, &shipped, &mut out, &mut err);
    let lines = |b: Vec<u8>| {
        String::from_utf8(b)
            .unwrap()
            .lines()
            .map(String::from)
            .collect()
    };
    (code, lines(out), lines(err))
}

#[test]
fn success_lists_each_member_and_why() {
    let (code, out, err) = run_main(&SHIPPED);
    assert_eq!(code, 0);
    assert_eq!(err, Vec::<String>::new());
    let line = "libgcc.a(pr-support.o) <- libsupc++.a(eh_personality.o) (__gnu_unwind_frame)";
    assert!(out.iter().any(|l| l == line), "{out:?}");
}

#[test]
fn a_mismatch_fails() {
    let (code, _, err) = run_main(&SHIPPED[1..]);
    assert_eq!(code, 1);
    assert!(
        err[0].contains("libsupc++.a(del_ops.o) is needed"),
        "{err:?}"
    );
}
