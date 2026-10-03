//! The Python tool's tests (tests/target2_abs32_test.py), case for case: the rewrite and
//! its refusals are symdev's `Target2Rewrite`, which these hold to the Python behaviour;
//! the file handling is `Target2Tool`'s.

mod elf_object;

use std::fs;

use symdev_elf2e32::Target2Rewrite;

use self::elf_object::*;
use super::Target2Tool;

fn rewrite(data: &[u8]) -> (Vec<u8>, usize) {
    let done = Target2Rewrite::object(data).unwrap();
    (done.bytes().to_vec(), done.rewritten())
}

#[test]
fn rewrites_every_target2_in_rel_and_rela_and_keeps_the_symbol() {
    let rels = rel(&[
        (0, 7, R_ARM_TARGET2),
        (4, 3, R_ARM_ABS32),
        (8, 0x1234, R_ARM_TARGET2),
    ]);
    let before = elf(
        &[
            (".text", SHT_PROGBITS, vec![0; 16], 0),
            (".rel.ARM.extab", SHT_REL, rels, 8),
            (
                ".rela.data",
                SHT_RELA,
                rela(&[(12, 9, R_ARM_TARGET2, -4)]),
                12,
            ),
        ],
        &GCCE,
    );
    let (data, count) = rewrite(&before);
    assert_eq!(count, 3);
    let abs = R_ARM_ABS32;
    assert_eq!(
        relocations(&data, 2, 8),
        [(0, 7, abs), (4, 3, abs), (8, 0x1234, abs)]
    );
    assert_eq!(relocations(&data, 3, 12), [(12, 9, abs)]);
    // Only the type bytes changed: two in .rel (its ABS32 was one already), one in .rela.
    let changed = before.iter().zip(&data).filter(|(a, b)| a != b).count();
    assert_eq!(changed, 2 + 1);
    assert_eq!(before.len(), data.len());
}

#[test]
fn an_object_without_target2_is_left_byte_for_byte() {
    let rels = rel(&[(0, 4, R_ARM_THM_CALL), (4, 5, R_ARM_ABS32)]);
    let before = elf(
        &[
            (".text", SHT_PROGBITS, vec![0; 8], 0),
            (".rel.text", SHT_REL, rels, 8),
        ],
        &GCCE,
    );
    assert_eq!(rewrite(&before), (before.clone(), 0));
}

#[test]
fn a_target2_byte_outside_a_relocation_section_is_not_touched() {
    // 0x29 in code and in the string table is not a relocation.
    let before = elf(
        &[(".text", SHT_PROGBITS, vec![R_ARM_TARGET2 as u8; 8], 0)],
        &GCCE,
    );
    assert_eq!(rewrite(&before), (before.clone(), 0));
}

fn refuses(data: &[u8], pattern: &str) {
    let error = Target2Rewrite::object(data).unwrap_err().to_string();
    assert!(
        error.contains(pattern),
        "{error:?} does not say {pattern:?}"
    );
}

#[test]
fn not_elf() {
    let mut data = b"!<arch>\n".to_vec();
    data.extend([0; 64]);
    refuses(&data, "not an ELF");
}

#[test]
fn elf64() {
    refuses(&elf(&[], &Ident { class: 2, ..GCCE }), "ELF32");
}

#[test]
fn big_endian() {
    refuses(&elf(&[], &Ident { data: 2, ..GCCE }), "little-endian");
}

#[test]
fn not_arm() {
    refuses(
        &elf(&[], &Ident { machine: 3, ..GCCE }),
        "machine 3, not ARM",
    );
}

#[test]
fn not_relocatable() {
    refuses(
        &elf(&[], &Ident { e_type: 2, ..GCCE }),
        "type 2, not a relocatable object",
    );
}

#[test]
fn a_relocation_section_with_the_wrong_entry_size() {
    let rels = rela(&[(0, 1, R_ARM_TARGET2, 0)]);
    refuses(
        &elf(&[(".rel.text", SHT_REL, rels, 12)], &GCCE),
        "entry size 12, expected 8",
    );
}

#[test]
fn a_relocation_section_past_the_end_of_the_file() {
    let mut data = elf(
        &[(".rel.text", SHT_REL, rel(&[(0, 1, R_ARM_TARGET2)]), 8)],
        &GCCE,
    );
    let shoff = u32::from_le_bytes(data[0x20..0x24].try_into().unwrap()) as usize;
    data[shoff + 40 + 20..shoff + 40 + 24].copy_from_slice(&4096u32.to_le_bytes()); // sh_size
    refuses(&data, "past the end");
}

#[test]
fn a_truncated_header() {
    refuses(b"\x7fELF\x01\x01\x01", "truncated");
}

#[test]
fn rewrites_files_in_place_and_reports_the_count_of_each() {
    let tmp = tempfile::tempdir().unwrap();
    let (a, b) = (tmp.path().join("a.o"), tmp.path().join("b.o"));
    fs::write(
        &a,
        elf(
            &[(".rel.x", SHT_REL, rel(&[(0, 1, R_ARM_TARGET2)]), 8)],
            &GCCE,
        ),
    )
    .unwrap();
    fs::write(&b, elf(&[(".text", SHT_PROGBITS, vec![0; 4], 0)], &GCCE)).unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(
        Target2Tool::run(&[a.clone(), b.clone()], &mut out, &mut err),
        0
    );
    let said = format!(
        "{}: 1 R_ARM_TARGET2 -> R_ARM_ABS32\n{}: 0 R_ARM_TARGET2 -> R_ARM_ABS32\n",
        a.display(),
        b.display()
    );
    assert_eq!(String::from_utf8(out).unwrap(), said);
    assert_eq!(
        relocations(&fs::read(&a).unwrap(), 1, 8),
        [(0, 1, R_ARM_ABS32)]
    );
}

#[test]
fn a_bad_file_fails_naming_it_and_leaves_it_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    let bad = tmp.path().join("bad.o");
    fs::write(&bad, b"not an object").unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(
        Target2Tool::run(std::slice::from_ref(&bad), &mut out, &mut err),
        1
    );
    let err = String::from_utf8(err).unwrap();
    assert!(
        err.starts_with(&format!("error: {}: not an ELF", bad.display())),
        "{err}"
    );
    assert_eq!(fs::read(&bad).unwrap(), b"not an object");
}
