#!/usr/bin/env bash
# Build emulator;2026.10.03 into <prefix>: the fork CI's EKA2L1 AppImage that artifact.toml names,
# extracted, checked and given its notices.
#
#   build.sh <absolute prefix>
#
# Runs in the current directory. The artifact's files come from EMULATOR_ARTIFACT_DIR when
# it is set, else from `gh run download` (GH_TOKEN must be able to read the fork's Actions).
# Their SHA-256s must be artifact.toml's: an artifact expires, the package does not, and a
# different file is never packed. The fork commit is cloned with its submodules into
# ./eka2l1-src (EKA2L1_GIT overrides where from), for the notices and for source.sh.
# PKGTOOLS names a pkgtools binary; by default this repository's runs through cargo.
set -euo pipefail
if [ $# -ne 1 ] || [ "${1#/}" = "$1" ]; then
  echo "usage: build.sh <absolute prefix>" >&2
  exit 2
fi
prefix=$1
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
pkgtools=${PKGTOOLS:-"cargo run --release --quiet --manifest-path $here/../../../Cargo.toml -p pkgtools --"}
value() { sed -n "s/^$1 = \"\(.*\)\"$/\1/p" "$2"; }
a=$here/artifact.toml
repository=$(value repository "$a"); commit=$(value commit "$a"); run=$(value run "$a")
artifact=$(value artifact "$a"); glibc=$(value glibc "$a")
appimage_sha=$(value appimage-sha256 "$a"); packages_sha=$(value packages-sha256 "$a")
id=$(value id "$here/recipe.toml")
for v in "$run" "$appimage_sha" ${packages_sha:+"$packages_sha"}; do
  case $v in
    *[!0]*) ;;
    *) echo "error: $a still has zeros: fill in the CI run and its hashes (plan Task 16)" >&2; exit 1 ;;
  esac
done

rm -rf artifact squashfs-root && mkdir artifact
if [ -n "${EMULATOR_ARTIFACT_DIR:-}" ]; then
  cp "$EMULATOR_ARTIFACT_DIR"/eka2l1-qt-x64.* artifact/
else
  gh run download "$run" -R "$repository" -n "$artifact" -D artifact
fi
{
  echo "$appimage_sha  eka2l1-qt-x64.AppImage"
  if [ -n "$packages_sha" ]; then echo "$packages_sha  eka2l1-qt-x64.packages.tsv"; fi
} > artifact/SHA256SUMS
(cd artifact && sha256sum -c SHA256SUMS)

chmod u+x artifact/eka2l1-qt-x64.AppImage
artifact/eka2l1-qt-x64.AppImage --appimage-extract > /dev/null
$pkgtools emulator-tree squashfs-root --glibc "$glibc"

# Shallow: the commit and each submodule's recorded commit only (a full clone with every
# submodule's history does not fit a runner's disk comfortably).
if [ ! -d eka2l1-src ]; then
  git init --quiet eka2l1-src
  git -C eka2l1-src remote add origin "${EKA2L1_GIT:-https://github.com/$repository}"
fi
git -C eka2l1-src fetch --quiet --depth 1 origin "$commit"
git -C eka2l1-src checkout --quiet --detach FETCH_HEAD
git -C eka2l1-src submodule update --init --recursive --depth 1 --quiet
[ "$(git -C eka2l1-src rev-parse HEAD)" = "$commit" ]

notices=(--id "$id" --commit "$commit")
if [ -n "$packages_sha" ]; then notices+=(--packages artifact/eka2l1-qt-x64.packages.tsv); fi
if [ -f "$here/notices-extra.txt" ]; then notices+=(--extra "$here/notices-extra.txt"); fi
$pkgtools emulator-notices eka2l1-src squashfs-root "${notices[@]}"
rm -rf "$prefix" && mkdir -p "$(dirname "$prefix")" && mv squashfs-root "$prefix"
echo "built $id in $prefix"
