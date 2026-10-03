#!/usr/bin/env python3
"""A case-insensitive overlay of the S60 SDK's epoc32/include for a case-sensitive host.

    sdk_casefold.py <epoc32/include> <out-dir>

The SDK was written on Windows, so its headers include each other in the wrong case
(`fbs.h` asks for <FbsMessage.h>, the file is `fbsmessage.h`). For every name an `#include`
line anywhere in the tree asks for that does not exist as written but does exist in another
case, <out-dir> gets a symlink by that name to the real file; the overlay goes on the include
path after the SDK's own directories. This is the rule of symdev's `SdkIncludeCaseFold`
(crates/symdev-build/src/resources/casefold.rs), which symdev applies when it compiles the
Avkon shim per application; the prebuilt shims are compiled with the same overlay so they see
the same headers. Where two files differ only in case, the first in sorted order wins.

An overlay already marked as built (<out-dir>/.symdev-casefold) is reused as it is.
"""

import os
import sys
from pathlib import Path

MARKER = ".symdev-casefold"


class CaseFoldError(Exception):
    pass


def ensure(include, out):
    # Absolute, so the links work from wherever the overlay is used.
    include, out = Path(include).absolute(), Path(out)
    if (out / MARKER).is_file():
        return out
    if not include.is_dir():
        raise CaseFoldError(f"{include} is not a directory")
    by_lower = {}
    names = set()
    for dirpath, dirnames, filenames in os.walk(include):
        dirnames.sort()
        for filename in sorted(filenames):
            path = Path(dirpath, filename)
            rel = path.relative_to(include).as_posix().replace("\\", "/")
            by_lower.setdefault(rel.lower(), path)
            names.update(include_names(path.read_bytes()))
    out.mkdir(parents=True, exist_ok=True)
    for name in sorted(names):
        if (include / name).exists():
            continue
        real = by_lower.get(name.lower())
        if real is None:
            continue
        link = out / name
        link.parent.mkdir(parents=True, exist_ok=True)
        if not link.is_symlink() and not link.exists():
            link.symlink_to(real)
    (out / MARKER).write_text(str(include))
    return out


def include_names(data):
    """The names the `#include <…>` / `#include "…"` lines of `data` ask for, `\\` as `/`;
    absolute names and names with `..` are left out."""
    found = set()
    for raw in data.split(b"\n"):
        line = raw.decode("utf-8", errors="replace").lstrip()
        if not line.startswith("#"):
            continue
        rest = line[1:].lstrip()
        if not rest.startswith("include"):
            continue
        rest = rest[len("include"):].lstrip()
        if not rest or rest[0] not in '<"':
            continue
        close = ">" if rest[0] == "<" else '"'
        end = rest.find(close, 1)
        if end < 0:
            continue
        name = rest[1:end].replace("\\", "/")
        if name and not name.startswith("/") and ".." not in name:
            found.add(name)
    return found


def main(argv):
    if len(argv) != 2:
        print("usage: sdk_casefold.py <epoc32/include> <out-dir>", file=sys.stderr)
        return 2
    try:
        print(ensure(argv[0], argv[1]))
    except (OSError, CaseFoldError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
