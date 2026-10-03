#!/usr/bin/env python3
"""Rewrite every R_ARM_TARGET2 relocation of ELF objects into R_ARM_ABS32, in place.

    target2_abs32.py <object.o>...

For the prebuilt shims of the rust-sdk package (symdev experiment 109, section 2, change 5).
On Symbian, R_ARM_TARGET2 means an absolute word: GNU ld's symbianelf target links it as
R_ARM_ABS32, and libsupc++'s personality routine, built for __symbian__, reads the catch
typeinfo word of an exception table as an absolute pointer. rust-lld can treat TARGET2 as
absolute (--target2=abs) but cannot emit a dynamic relocation for it against an imported
symbol, which the `typeinfo for XLeaveException` of every TRAP needs. With the type rewritten
the object means the same to GNU ld and lld links it.

Only the type byte of each relocation entry changes (ELF32_R_INFO keeps its symbol), so no
size or offset moves. Accepted input is exactly what GCCE writes: ELF32, little-endian,
EM_ARM, ET_REL. Anything else is refused and the file is left as it was.
"""

import struct
import sys

R_ARM_ABS32 = 2
R_ARM_TARGET2 = 41
SHT_RELA, SHT_REL = 4, 9
ENTRY_SIZE = {SHT_REL: 8, SHT_RELA: 12}
EM_ARM, ET_REL = 40, 1
EHDR_SIZE, SHDR_SIZE = 52, 40


class Target2Error(Exception):
    pass


def rewrite(data):
    """Rewrite the TARGET2 relocations of one object held in the bytearray `data`;
    returns how many. Raises Target2Error, with `data` untouched, if it is not an object
    this tool knows."""
    sections = _relocation_sections(data)
    count = 0
    for offset, size, entsize in sections:
        for entry in range(offset, offset + size, entsize):
            if data[entry + 4] == R_ARM_TARGET2:
                data[entry + 4] = R_ARM_ABS32
                count += 1
    return count


def _relocation_sections(data):
    """(offset, size, entsize) of every SHT_REL/SHT_RELA section, all checked first."""
    if len(data) < EHDR_SIZE:
        if data[:4] == b"\x7fELF":
            raise Target2Error("truncated ELF header")
        raise Target2Error("not an ELF file")
    if data[:4] != b"\x7fELF":
        raise Target2Error("not an ELF file")
    if data[4] != 1:
        raise Target2Error(f"ELF class {data[4]}, not ELF32")
    if data[5] != 1:
        raise Target2Error("not a little-endian ELF")
    e_type, machine = struct.unpack_from("<HH", data, 16)
    if machine != EM_ARM:
        raise Target2Error(f"machine {machine}, not ARM ({EM_ARM})")
    if e_type != ET_REL:
        raise Target2Error(f"type {e_type}, not a relocatable object")
    shoff = struct.unpack_from("<I", data, 0x20)[0]
    shentsize, shnum = struct.unpack_from("<HH", data, 0x2E)
    if shnum == 0:
        if shoff:
            raise Target2Error("extended section numbering is not supported")
        return []
    if shentsize != SHDR_SIZE:
        raise Target2Error(f"section header size {shentsize}, expected {SHDR_SIZE}")
    if shoff + shnum * SHDR_SIZE > len(data):
        raise Target2Error("section header table past the end of the file")
    found = []
    for index in range(shnum):
        header = shoff + index * SHDR_SIZE
        kind = struct.unpack_from("<I", data, header + 4)[0]
        if kind not in ENTRY_SIZE:
            continue
        offset, size, _, _, _, entsize = struct.unpack_from("<6I", data, header + 16)
        expected = ENTRY_SIZE[kind]
        if entsize != expected:
            raise Target2Error(f"section {index}: entry size {entsize}, expected {expected}")
        if size % expected:
            raise Target2Error(f"section {index}: size {size} is not a whole number of entries")
        if offset + size > len(data):
            raise Target2Error(f"section {index}: relocations past the end of the file")
        found.append((offset, size, expected))
    return found


def main(argv, out=print, err=lambda line: print(line, file=sys.stderr)):
    if not argv:
        err("usage: target2_abs32.py <object.o>...")
        return 2
    for path in argv:
        with open(path, "rb") as f:
            data = bytearray(f.read())
        try:
            count = rewrite(data)
        except Target2Error as e:
            err(f"error: {path}: {e}")
            return 1
        if count:
            with open(path, "wb") as f:
                f.write(data)
        out(f"{path}: {count} R_ARM_TARGET2 -> R_ARM_ABS32")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
