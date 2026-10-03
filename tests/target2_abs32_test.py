"""Tests of tools/target2_abs32.py on ELF objects built byte by byte here.

    python3 -m unittest discover -s tests -p '*_test.py'
"""

import struct
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "tools"))
import target2_abs32 as t2  # noqa: E402

SHT_PROGBITS, SHT_STRTAB, SHT_RELA, SHT_REL = 1, 3, 4, 9
R_ARM_ABS32, R_ARM_THM_CALL, R_ARM_TARGET2 = 2, 10, 41


def elf(sections, *, ident_class=1, ident_data=1, e_type=1, machine=40):
    """An ELF32 relocatable object: a null section, then `sections`, each
    (name, type, payload, entsize), then .shstrtab. Payloads follow the 52-byte header,
    the section header table comes last."""
    names = b"\0" + b"".join(name.encode() + b"\0" for name, *_ in sections) + b".shstrtab\0"
    body = bytearray()
    headers = [struct.pack("<10I", 0, 0, 0, 0, 0, 0, 0, 0, 0, 0)]
    name_off = 1
    for name, kind, payload, entsize in sections + [(".shstrtab", SHT_STRTAB, names, 0)]:
        offset = 52 + len(body)
        body += payload
        headers.append(struct.pack("<10I", name_off, kind, 0, 0, offset, len(payload),
                                   0, 0, 4, entsize))
        name_off += len(name) + 1
    shoff = 52 + len(body)
    endian = "<" if ident_data == 1 else ">"
    ident = b"\x7fELF" + bytes([ident_class, ident_data, 1]) + bytes(9)
    header = ident + struct.pack(endian + "HHIIIIIHHHHHH", e_type, machine, 1, 0, 0, shoff,
                                 0x05000000, 52, 0, 0, 40, len(headers), len(headers) - 1)
    return bytearray(header + body + b"".join(headers))


def rel(*entries):
    return b"".join(struct.pack("<II", offset, (sym << 8) | kind)
                    for offset, sym, kind in entries)


def rela(*entries):
    return b"".join(struct.pack("<IIi", offset, (sym << 8) | kind, addend)
                    for offset, sym, kind, addend in entries)


def relocations(data, section_index, step):
    """(offset, sym, type) of every entry of the section at `section_index`."""
    shoff = struct.unpack_from("<I", data, 0x20)[0]
    off, size = struct.unpack_from("<II", data, shoff + section_index * 40 + 16)
    return [(o, info >> 8, info & 0xFF) for o, info in
            (struct.unpack_from("<II", data, p) for p in range(off, off + size, step))]


class RewriteTest(unittest.TestCase):
    def test_rewrites_every_target2_in_rel_and_rela_and_keeps_the_symbol(self):
        data = elf([
            (".text", SHT_PROGBITS, bytes(16), 0),
            (".rel.ARM.extab", SHT_REL, rel((0, 7, R_ARM_TARGET2), (4, 3, R_ARM_ABS32),
                                            (8, 0x1234, R_ARM_TARGET2)), 8),
            (".rela.data", SHT_RELA, rela((12, 9, R_ARM_TARGET2, -4)), 12),
        ])
        before = bytes(data)
        self.assertEqual(t2.rewrite(data), 3)
        self.assertEqual(relocations(data, 2, 8),
                         [(0, 7, R_ARM_ABS32), (4, 3, R_ARM_ABS32), (8, 0x1234, R_ARM_ABS32)])
        self.assertEqual(relocations(data, 3, 12), [(12, 9, R_ARM_ABS32)])
        # Only the type bytes changed: two in .rel (its ABS32 was one already), one in .rela.
        changed = [i for i, (a, b) in enumerate(zip(before, data)) if a != b]
        self.assertEqual(len(changed), 2 + 1)
        self.assertEqual(len(before), len(data))

    def test_an_object_without_target2_is_left_byte_for_byte(self):
        data = elf([
            (".text", SHT_PROGBITS, bytes(8), 0),
            (".rel.text", SHT_REL, rel((0, 4, R_ARM_THM_CALL), (4, 5, R_ARM_ABS32)), 8),
        ])
        before = bytes(data)
        self.assertEqual(t2.rewrite(data), 0)
        self.assertEqual(bytes(data), before)

    def test_a_target2_byte_outside_a_relocation_section_is_not_touched(self):
        # 0x29 in code and in the string table is not a relocation.
        data = elf([(".text", SHT_PROGBITS, bytes([R_ARM_TARGET2]) * 8, 0)])
        before = bytes(data)
        self.assertEqual(t2.rewrite(data), 0)
        self.assertEqual(bytes(data), before)


class RefusalTest(unittest.TestCase):
    def refuses(self, data, pattern):
        with self.assertRaisesRegex(t2.Target2Error, pattern):
            t2.rewrite(bytearray(data))

    def test_not_elf(self):
        self.refuses(b"!<arch>\n" + bytes(64), "not an ELF")

    def test_elf64(self):
        self.refuses(elf([], ident_class=2), "ELF32")

    def test_big_endian(self):
        self.refuses(elf([], ident_data=2), "little-endian")

    def test_not_arm(self):
        self.refuses(elf([], machine=3), "machine 3, not ARM")

    def test_not_relocatable(self):
        self.refuses(elf([], e_type=2), "type 2, not a relocatable object")

    def test_a_relocation_section_with_the_wrong_entry_size(self):
        self.refuses(elf([(".rel.text", SHT_REL, rela((0, 1, R_ARM_TARGET2, 0)), 12)]),
                     "entry size 12, expected 8")

    def test_a_relocation_section_past_the_end_of_the_file(self):
        data = elf([(".rel.text", SHT_REL, rel((0, 1, R_ARM_TARGET2)), 8)])
        shoff = struct.unpack_from("<I", data, 0x20)[0]
        struct.pack_into("<I", data, shoff + 40 + 20, 4096)  # sh_size
        self.refuses(data, "past the end")

    def test_a_truncated_header(self):
        self.refuses(b"\x7fELF\x01\x01\x01", "truncated")


class MainTest(unittest.TestCase):
    def test_rewrites_files_in_place_and_reports_the_count_of_each(self):
        with tempfile.TemporaryDirectory() as tmp:
            a, b = Path(tmp, "a.o"), Path(tmp, "b.o")
            a.write_bytes(elf([(".rel.x", SHT_REL, rel((0, 1, R_ARM_TARGET2)), 8)]))
            b.write_bytes(elf([(".text", SHT_PROGBITS, bytes(4), 0)]))
            lines = []
            self.assertEqual(t2.main([str(a), str(b)], out=lines.append), 0)
            self.assertEqual(lines, [f"{a}: 1 R_ARM_TARGET2 -> R_ARM_ABS32",
                                     f"{b}: 0 R_ARM_TARGET2 -> R_ARM_ABS32"])
            self.assertEqual(relocations(bytearray(a.read_bytes()), 1, 8),
                             [(0, 1, R_ARM_ABS32)])

    def test_a_bad_file_fails_naming_it_and_leaves_it_unchanged(self):
        with tempfile.TemporaryDirectory() as tmp:
            bad = Path(tmp, "bad.o")
            bad.write_bytes(b"not an object")
            errors = []
            self.assertEqual(t2.main([str(bad)], out=lambda _: None, err=errors.append), 1)
            self.assertRegex(errors[0], f"{bad}: not an ELF")
            self.assertEqual(bad.read_bytes(), b"not an object")


if __name__ == "__main__":
    unittest.main()
