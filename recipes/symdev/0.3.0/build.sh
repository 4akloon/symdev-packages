#!/usr/bin/env bash
# Build symdev;<ver> and rust-sdk;<ver> from the tag recipe.toml names.
#
#   build.sh <out-dir>
#
# Runs in the current directory: clones recipe.toml's `git` at `tag` into ./symdev, checks
# that the tag's workspace version is the version of the recipe's ids, builds a static
# x86_64 musl binary and fails unless it is static. Writes into <out-dir>:
#
#   symdev/                       bin/symdev, and share/doc/symdev/ with LICENSE and
#                                 THIRD-PARTY-NOTICES.txt (tools/third_party_notices.py of
#                                 this repository: what the static binary links), for
#                                 publish public 'symdev;<ver>' --from <out-dir>/symdev
#   rust-sdk/                     the tag's tree (git archive); recipe.toml's include list
#                                 takes the Rust SDK out of it, for
#                                 publish public 'rust-sdk;<ver>' --from <out-dir>/rust-sdk
#   symdev-<ver>-source.tar.gz    git archive of the tag: both packages' --source-code
#
# rust-sdk/symbian-rs/prebuilt/ is not made here: prebuilt.sh makes it afterwards, with GCCE
# and the S60 SDK that this build's symdev installs (symdev.yml runs the three in turn, the
# install alone with the private source's reader key).
#
# Needs git, cargo with the x86_64-unknown-linux-musl target, a musl C compiler for ring
# and libz-sys (cc-rs finds musl-gcc, Debian/Ubuntu package musl-tools), file, ldd, python3
# and ar. Runs from its place in the repository (it calls ../../../tools/).
# CARGO_TARGET_DIR is honoured.
set -euo pipefail

if [ $# -ne 1 ]; then
  echo "usage: build.sh <out-dir>" >&2
  exit 2
fi
mkdir -p "$1"
out=$(cd "$1" && pwd)
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
notices=$here/../../../tools/third_party_notices.py
recipe=$here/recipe.toml
target=x86_64-unknown-linux-musl

fail() {
  echo "error: $*" >&2
  exit 1
}

# `key = "value"` at the top of recipe.toml (before the first table).
top() {
  awk -v key="$1" '/^\[/ { exit } $1 == key && $2 == "=" {
    v = $3; gsub(/^"|"$/, "", v); print v; exit }' "$recipe"
}
git_url=$(top git)
tag=$(top tag)
[ -n "$git_url" ] && [ -n "$tag" ] || fail "$recipe names no git and tag"

# The version every id of the recipe carries: `symdev;<ver>` and `rust-sdk;<ver>`.
version=$(sed -n 's/^id = "symdev;\(.*\)"$/\1/p' "$recipe")
[ -n "$version" ] || fail "$recipe has no symdev;<ver> package"
grep -qx "id = \"rust-sdk;$version\"" "$recipe" ||
  fail "$recipe has no rust-sdk;$version package beside symdev;$version"

# --- source ----------------------------------------------------------------------------
src=$PWD/symdev
rm -rf "$src"
git -c advice.detachedHead=false clone --quiet --depth 1 --branch "$tag" "$git_url" \
  "$src" || fail "cannot clone $git_url at $tag (does the tag exist?)"
commit=$(git -C "$src" rev-parse HEAD)
[ "$(git -C "$src" rev-parse "refs/tags/$tag^{commit}")" = "$commit" ] ||
  fail "$tag in $git_url is not a tag"
workspace=$(awk '/^\[/ { section = $0 } section == "[workspace.package]" &&
  $1 == "version" { v = $3; gsub(/"/, "", v); print v; exit }' "$src/Cargo.toml")
[ "$workspace" = "$version" ] ||
  fail "$tag's workspace version is '$workspace', but $recipe builds $version"
echo "symdev $version: $git_url $tag = $commit"
# rust-sdk takes all of symbian-rs but corpus/ (recipe.toml says why); an entry a later tag
# adds must not be left out without a word.
for entry in $(git -C "$src" ls-tree --name-only "$tag" symbian-rs/); do
  [ "$entry" != symbian-rs/corpus ] || continue
  grep -qF "\"$entry\"" "$recipe" ||
    fail "$tag has $entry, which the rust-sdk include list of $recipe does not name; add it"
done

# --- static binary ---------------------------------------------------------------------
rustc -vV
# SYMDEV_RELEASE=1, read by symdev at compile time (option_env!), removes its fallback to the
# source checkout it was built from when it looks for the Rust SDK: on a user's machine that
# path (this build's $src) is just a directory a local user could create, holding a planted
# symbian-rs. A released symdev finds the SDK only through SYMDEV_RUST_SDK or the installed
# rust-sdk package.
# LIBZ_SYS_STATIC=1 makes libz-sys compile its bundled zlib (the one THIRD-PARTY-NOTICES.txt
# names) with the musl compiler. Without it libz-sys links any libz its `-lz` test finds,
# and with musl-gcc or the host cc that can be the build host's glibc-built libz.a (seen in
# a local build, 2026-10-02).
(cd "$src" && SYMDEV_RELEASE=1 LIBZ_SYS_STATIC=1 \
  cargo build --release --locked -p symdev-cli --target "$target")
release=${CARGO_TARGET_DIR:-$src/target}/$target/release
bin=$release/symdev
# What libz-sys' build script told cargo this time (the newest of its output files).
zlib=$(ls -t "$release"/build/libz-sys-*/output 2>/dev/null | head -n 1 || true)
[ -n "$zlib" ] && grep -qx 'cargo:rustc-link-lib=static=z' "$zlib" ||
  fail "libz-sys did not link its bundled zlib statically (${zlib:-no build output}); is LIBZ_SYS_STATIC=1 reaching it?"
kind=$(file -bL "$bin")
case $kind in
  *"statically linked"* | *"static-pie linked"*) ;;
  *) fail "$bin is not statically linked: $kind" ;;
esac
# glibc's ldd says "statically linked" or "not a dynamic executable" (exit 1) for a static
# binary; a library line has "=>" or names the dynamic loader.
libs=$(ldd "$bin" 2>&1 || true)
if printf '%s\n' "$libs" | grep -q -e '=>' -e 'ld-linux'; then
  fail "$bin needs shared libraries: $libs"
fi
echo "$bin: $kind; ldd: $libs"
"$bin" --help >/dev/null

# --- outputs ---------------------------------------------------------------------------
rm -rf "$out/symdev" "$out/rust-sdk"
mkdir -p "$out/symdev/bin" "$out/rust-sdk"
cp "$bin" "$out/symdev/bin/symdev"
chmod 0755 "$out/symdev/bin/symdev"
doc=$out/symdev/share/doc/symdev
mkdir -p "$doc"
cp "$src/LICENSE" "$doc/LICENSE"
python3 "$notices" --manifest-path "$src/Cargo.toml" --package symdev-cli --target "$target" \
  --output "$doc/THIRD-PARTY-NOTICES.txt" || fail "cannot write $doc/THIRD-PARTY-NOTICES.txt"
git -C "$src" archive "$tag" | tar -x -C "$out/rust-sdk"
git -C "$src" archive --format=tar.gz --prefix="symdev-$version/" \
  -o "$out/symdev-$version-source.tar.gz" "$tag"
ls -l "$out/symdev/bin/symdev" "$doc" "$out/symdev-$version-source.tar.gz"
