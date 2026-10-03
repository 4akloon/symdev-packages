#!/usr/bin/env python3
"""Fail unless build outputs hold nothing of the S60 SDK.

    sdk_free.py <sdk-dir> <path>...

The S60 SDK's licence does not let it be redistributed, yet symdev.yml installs it on the
runner to compile rust-sdk's prebuilt shims (recipes/symdev/<ver>/prebuilt.sh). Before the
build is handed on as an artifact, this reads every file under each <path> (a directory, a
tar or a .tar.gz), and every member of every tar or .tar.gz found there, nested ones too,
and reports each that

  - has the same bytes as a file of <sdk-dir> (empty files excepted), whatever its name, or
  - has a path through a directory named epoc32 (in any case): the SDK's own layout.

Exit 0 when there is none, 1 when there is any (each on stderr), 2 when the check cannot
be made: a <path> that does not exist or cannot be read, or an <sdk-dir> with no file to
compare against, so the check never passes by looking at nothing. What the shims compile
in from the SDK's headers (inline code, constants) is not a file of the SDK and is not
looked for.
"""

import hashlib
import io
import os
import sys
import tarfile
from pathlib import Path, PurePosixPath


class SdkFreeError(Exception):
    pass


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def through_epoc32(name):
    return any(part.lower() == "epoc32" for part in PurePosixPath(name).parts)


class SdkFiles:
    """The SHA-256 of every non-empty regular file of an SDK tree, to its path."""

    def __init__(self, root, by_hash):
        self.root = root
        self.by_hash = by_hash

    @classmethod
    def of(cls, root):
        root = Path(root)
        if not root.is_dir():
            raise SdkFreeError(f"{root} is not a directory")
        by_hash = {}
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames.sort()
            for filename in sorted(filenames):
                path = Path(dirpath, filename)
                if path.is_symlink() or not path.is_file():
                    continue
                data = path.read_bytes()
                if data:
                    by_hash.setdefault(sha256(data), str(path.relative_to(root)))
        if not by_hash:
            raise SdkFreeError(f"{root} has no file to compare against: is it the SDK?")
        return cls(root, by_hash)

    def leaks(self, paths):
        """A line for each file under `paths` that comes from the SDK, in the order found."""
        found = []
        for path in paths:
            path = Path(path)
            if path.is_dir():
                for dirpath, dirnames, filenames in os.walk(path):
                    dirnames.sort()
                    for filename in sorted(filenames):
                        file = Path(dirpath, filename)
                        rel = file.relative_to(path).as_posix()
                        self._file(str(file), rel, file.read_bytes(), found)
            elif path.is_file():
                self._file(str(path), "", path.read_bytes(), found)
            else:
                raise SdkFreeError(f"{path} does not exist")
        return found

    def _file(self, name, rel, data, found):
        """`name` is shown; `rel` is the part below what the caller named (a member's
        name, a path inside a directory), which alone is checked for epoc32/."""
        if through_epoc32(rel):
            found.append(f"{name}: a path through epoc32/")
        source = self.by_hash.get(sha256(data)) if data else None
        if source:
            found.append(f"{name}: the bytes of the SDK's {source}")
        if name.endswith((".tar", ".tar.gz", ".tgz")):
            self._archive(name, data, found)

    def _archive(self, name, data, found):
        try:
            with tarfile.open(fileobj=io.BytesIO(data), mode="r:*") as tar:
                for member in tar:
                    inner = f"{name}!{member.name}"
                    if member.isfile():
                        extracted = tar.extractfile(member)
                        self._file(inner, member.name, extracted.read() if extracted else b"", found)
                    elif through_epoc32(member.name):
                        found.append(f"{inner}: a path through epoc32/")
        except (tarfile.TarError, EOFError, OSError) as e:
            raise SdkFreeError(f"cannot read {name} as a tar: {e}") from e


def main(argv):
    if len(argv) < 3:
        print("usage: sdk_free.py <sdk-dir> <path>...", file=sys.stderr)
        return 2
    try:
        sdk = SdkFiles.of(argv[1])
        leaks = sdk.leaks(argv[2:])
    except (SdkFreeError, OSError) as e:
        print(f"error: {e}", file=sys.stderr)
        return 2
    for leak in leaks:
        print(f"error: {leak}", file=sys.stderr)
    if leaks:
        print(f"error: {len(leaks)} file(s) of the S60 SDK in {' '.join(argv[2:])}",
              file=sys.stderr)
        return 1
    print(f"no file of the SDK ({len(sdk.by_hash)} distinct files) in {' '.join(argv[2:])}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
