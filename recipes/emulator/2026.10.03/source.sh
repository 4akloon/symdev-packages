#!/usr/bin/env bash
# The corresponding source of emulator;2026.10.03, one archive for `publish public --source-code`:
#   <name>/eka2l1/     the fork commit and every submodule at its recorded commit (git archive)
#   <name>/recipe/     this recipe directory
#   <name>/ubuntu/<source>/   each Ubuntu source package of the artifact's package list at its
#                      exact version, from Launchpad, checked against its .dsc (plan D1 = A)
#   <name>/SHA256SUMS  of every file above
#
#   source.sh <out.tar.gz>      (in build.sh's directory, after build.sh)
set -euo pipefail
if [ $# -ne 1 ]; then echo "usage: source.sh <out.tar.gz>" >&2; exit 2; fi
out=$1
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
pkgtools=${PKGTOOLS:-"cargo run --release --quiet --manifest-path $here/../../../Cargo.toml -p pkgtools --"}
id=$(sed -n 's/^id = "\(.*\)"$/\1/p' "$here/recipe.toml")
name="${id//;/-}-source"
[ -d eka2l1-src ] || { echo "error: no ./eka2l1-src: run build.sh here first" >&2; exit 1; }
rm -rf src-out && mkdir -p "src-out/$name/eka2l1" "src-out/$name/recipe"
(cd eka2l1-src && git archive --format=tar HEAD &&
  git submodule foreach --quiet --recursive 'git archive --format=tar --prefix="$displaypath/" HEAD') |
  tar -x -i -C "src-out/$name/eka2l1"
cp -R "$here/." "src-out/$name/recipe/"
lp=https://launchpad.net/ubuntu/+archive/primary/+sourcefiles
if [ -f artifact/eka2l1-qt-x64.packages.tsv ]; then
  cut -f3,4 artifact/eka2l1-qt-x64.packages.tsv | sort -u | while IFS=$'\t' read -r src ver; do
    dir="src-out/$name/ubuntu/$src"
    dsc="${src}_${ver#*:}.dsc"
    mkdir -p "$dir"
    curl -fsSL --retry 3 -o "$dir/$dsc" "$lp/$src/$ver/$dsc"
    $pkgtools dsc-files "$dir/$dsc" > "$dir/SHA256SUMS"
    while read -r _ file; do
      curl -fsSL --retry 3 -o "$dir/$file" "$lp/$src/$ver/$file"
    done < "$dir/SHA256SUMS"
    (cd "$dir" && sha256sum -c --quiet SHA256SUMS)
  done
fi
(cd "src-out/$name" && find . -type f ! -path ./SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS)
tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner -C src-out -cf - "$name" | gzip -n -9 > "$out"
echo "wrote $out ($(stat -c %s "$out") bytes)"
