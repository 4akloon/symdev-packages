#!/usr/bin/env bash
# Stage firmware;rm-469;1 from an EKA2L1 data folder, for `publish private`:
#
#   EKA2L1_DATA=~/.local/share/EKA2L1/data bash stage.sh <out-dir>
#
# <out-dir> must not exist. It gets roms/rm-469/ and drives/z/rm-469/ copied as they are,
# and device.yml, the RM-469 entry of $EKA2L1_DATA/devices.yml (pkgtools device-entry).
# Nothing in EKA2L1_DATA is written. PKGTOOLS names a pkgtools binary; by default this
# repository's is run through cargo.
set -euo pipefail
if [ $# -ne 1 ]; then
  echo "usage: EKA2L1_DATA=<EKA2L1 data folder> stage.sh <out-dir>" >&2
  exit 2
fi
data=${EKA2L1_DATA:?set EKA2L1_DATA to the EKA2L1 data folder that holds devices.yml, roms/ and drives/}
out=$1
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
pkgtools=${PKGTOOLS:-"cargo run --release --quiet --manifest-path $here/../../../../Cargo.toml -p pkgtools --"}
if [ -e "$out" ]; then
  echo "error: $out exists; stage.sh writes a new folder" >&2
  exit 1
fi
for part in devices.yml roms/rm-469 drives/z/rm-469; do
  if [ ! -e "$data/$part" ]; then
    echo "error: $data/$part is missing; install the RM-469 firmware in EKA2L1 first" >&2
    exit 1
  fi
done
mkdir -p "$out/roms" "$out/drives/z"
$pkgtools device-entry "$data/devices.yml" RM-469 > "$out/device.yml"
cp -a "$data/roms/rm-469" "$out/roms/"
cp -a "$data/drives/z/rm-469" "$out/drives/z/"
echo "staged firmware;rm-469;1 in $out"
