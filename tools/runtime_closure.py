#!/usr/bin/env python3
"""Check that the GCC runtime members a link pulled are exactly the ones the package ships.

    runtime_closure.py <ld.map> <archive>(<member>)...

<ld.map> is the -Map of a GNU ld link of every prebuilt shim (--whole-archive) against the
SDK import libraries and GCCE's full libsupc++.a and libgcc.a, in the order symdev's link
line has them. Its "Archive member included to satisfy reference by file (symbol)" section
says which members the linker took and for whom (symdev experiment 109, section 1, did the
same with lld's --why-extract). Each argument names a member the rust-sdk package ships,
e.g. `libgcc.a(pr-support.o)`; only the archives these arguments name are checked.

Exit 1, naming member, referrer and symbol, when a shim needs a member that is not shipped
(a changed shim cannot silently depend on runtime code the package lacks) or when a shipped
member is needed by nothing (the set, and its licence, stays exactly what the shims use).
"""

import os
import re
import sys
from dataclasses import dataclass

HEADERS = (
    "Archive member included to satisfy reference by file (symbol)",
    "Archive member included because of file (symbol)",  # older binutils
)
MEMBER = re.compile(r"^(?P<path>.+)\((?P<member>[^()]+)\)$")


class ClosureError(Exception):
    pass


@dataclass
class Inclusion:
    path: str  # the archive as the map spells it
    member: str
    referrer: str | None  # None: an -u / entry symbol of the command line
    symbol: str

    @property
    def archive(self):
        return os.path.basename(self.path)

    def name(self):
        return f"{self.archive}({self.member})"

    def reason(self):
        referrer = "the command line" if self.referrer is None else short(self.referrer)
        return f"{referrer} ({self.symbol})"


def short(file):
    """`/a/b/libx.a(m.o)` -> `libx.a(m.o)`, `/a/b/c.o` -> `c.o`."""
    match = MEMBER.match(file)
    if match:
        return f"{os.path.basename(match['path'])}({match['member']})"
    return os.path.basename(file)


def included_members(text):
    """Every archive member the map's inclusion section lists, in order."""
    lines = text.splitlines()
    try:
        start = next(i for i, line in enumerate(lines) if line.strip() in HEADERS)
    except StopIteration:
        raise ClosureError("no 'Archive member included' section in the map") from None
    found = []
    pending = None  # a member line still waiting for its reference line
    for number in range(start + 2, len(lines)):
        line = lines[number]
        if not line.strip():
            break
        if not line[0].isspace():
            if pending:
                raise ClosureError(f"map line {number}: member without a reference")
            head, _, rest = line.partition(" ")
            match = MEMBER.match(head)
            if not match:
                raise ClosureError(f"map line {number + 1}: not an archive member: {line!r}")
            pending = (match["path"], match["member"])
            if rest.strip():
                found.append(Inclusion(*pending, *reference(rest, number)))
                pending = None
        else:
            if not pending:
                raise ClosureError(f"map line {number + 1}: a reference with no member before it")
            found.append(Inclusion(*pending, *reference(line, number)))
            pending = None
    if pending:
        raise ClosureError("map ends with a member without a reference")
    return found


def reference(text, number):
    """`referrer (symbol)` or `(symbol)` -> (referrer or None, symbol)."""
    text = text.strip()
    if text.startswith("("):
        referrer, symbol = None, text
    else:
        referrer, _, symbol = text.partition(" ")
    if not (symbol.startswith("(") and symbol.endswith(")")):
        raise ClosureError(f"map line {number + 1}: no (symbol) in {text!r}")
    return referrer, symbol[1:-1]


def check(inclusions, shipped):
    """Problems, empty when the members `inclusions` took from the archives `shipped`
    names are exactly `shipped`."""
    wanted = set()
    for name in shipped:
        match = MEMBER.match(name)
        if not match or "/" in match["path"]:
            raise ClosureError(f"{name!r} is not <archive>(<member>), e.g. libgcc.a(pr-support.o)")
        wanted.add(name)
    archives = {MEMBER.match(name)["path"] for name in wanted}
    taken = {}
    for inclusion in inclusions:
        if inclusion.archive in archives:
            taken.setdefault(inclusion.name(), inclusion)
    problems = []
    for name, inclusion in taken.items():
        if name not in wanted:
            problems.append(f"{name} is needed by {inclusion.reason()} but not shipped: add it "
                            "to the prebuilt runtime set (and check its licence)")
    for name in shipped:
        if name not in taken:
            problems.append(f"{name} is shipped but no shim needs it")
    return problems


def main(argv, out=print, err=lambda line: print(line, file=sys.stderr)):
    if len(argv) < 2:
        err("usage: runtime_closure.py <ld.map> <archive>(<member>)...")
        return 2
    try:
        with open(argv[0], encoding="utf-8") as f:
            inclusions = included_members(f.read())
        problems = check(inclusions, argv[1:])
    except (OSError, ClosureError) as e:
        err(f"error: {argv[0]}: {e}")
        return 1
    archives = {name.split("(")[0] for name in argv[1:]}
    for inclusion in inclusions:
        if inclusion.archive in archives:
            out(f"{inclusion.name()} <- {inclusion.reason()}")
    for problem in problems:
        err(f"error: {problem}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
