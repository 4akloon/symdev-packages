#!/usr/bin/env bash
# Build the rust-sdk package's prebuilt/: symdev's C++ shims compiled once for every Rust
# application, and the four GCC runtime members they need, so that a Rust application can
# be linked by rust-lld with no GCCE on the machine (symdev experiment 109).
#
#   prebuilt.sh <symdev-src> <gcce> <epocroot> <out-dir>
#
#   <symdev-src>  the tag's tree (build.sh's ./symdev): the shims are its symbian-rs/shims
#   <gcce>        the installed gcce;12.1.0 ($SYMDEV_HOME/gcce/12.1.0)
#   <epocroot>    the installed S60 SDK, the directory holding epoc32/
#                 ($SYMDEV_HOME/sdk/s60-3rd-fp2/1.1)
#   <out-dir>     normally <build.sh's out-dir>/rust-sdk/symbian-rs/prebuilt; gets
#
#     lib/libsymrs.a      shims/common/*.cpp      every Rust application    } MIT, compiled
#     lib/libsymrs_ui.a   shims/s60/*.cpp         an `[ui]` (Avkon) one     } against the SDK
#     lib/libsupc++.a     del_ops.o eh_personality.o   } GPL-3.0-or-later WITH
#     lib/libgcc.a        pr-support.o _thumb1_case_uqi.o } GCC-exception-3.1
#     NOTICE, COPYING3, COPYING.RUNTIME
#
# Work files go to ./prebuilt-work. Needs PKGTOOLS, the path of this repository's pkgtools
# binary (cargo build --release --locked -p pkgtools), and nothing from the build host's own
# compilers: every tool is GCCE's. Steps:
#
# 1. Each shim source is compiled with the C++ argv symdev's GNU build gives it (GcceBuild's
#    recorded line, crates/symdev-build/src/driver/compile.rs, plus RustBuild::SHIM_OPTIONS
#    and the case-fold overlay of driver/rust_shims.rs) — the line of an `[ui]` build, less
#    its -DSYMRS_UID3: without it symrs_avkon.cpp reads the UID3 from the symbol
#    `symrs_uid3`, which the application's link defines (--defsym=symrs_uid3=0x<uid3>).
#    The objects are byte-identical to the ones symdev compiles per application.
# 2. R_ARM_TARGET2 -> R_ARM_ABS32 in every object (`pkgtools target2-abs32`, symdev's own
#    Target2Rewrite; experiment 109 says why).
# 3. The runtime members are taken out of GCCE's own libsupc++.a and libgcc.a and stripped
#    of debug information.
# 4. Closure: every shim, --whole-archive, linked by GCCE's ld against the SDK import
#    libraries and GCCE's full libsupc++.a/libgcc.a must take exactly the members of step 3
#    (`pkgtools runtime-closure`), and the same link against only lib/ must leave exactly the
#    same symbols undefined. A shim that starts to need another runtime member fails here.
set -euo pipefail

if [ $# -ne 4 ]; then
  echo "usage: prebuilt.sh <symdev-src> <gcce> <epocroot> <out-dir>" >&2
  exit 2
fi
src=$(cd "$1" && pwd)
gcce=$(cd "$2" && pwd)
epocroot=$(cd "$3" && pwd)
mkdir -p "$4"
out=$(cd "$4" && pwd)
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
tools=$here/../../../tools
work=$PWD/prebuilt-work

fail() {
  echo "error: $*" >&2
  exit 1
}

pkgtools=${PKGTOOLS:-}
[ -n "$pkgtools" ] && [ -x "$pkgtools" ] ||
  fail "PKGTOOLS='$pkgtools' is not this repository's pkgtools binary; build it with cargo build --release --locked -p pkgtools and set PKGTOOLS=<repository>/target/release/pkgtools"

target=arm-none-symbianelf
gcc_version=12.1.0
cxx=$gcce/bin/$target-g++
ar=$gcce/bin/$target-ar
ld=$gcce/bin/$target-ld
nm=$gcce/bin/$target-nm
objcopy=$gcce/bin/$target-objcopy
gcc_lib=$gcce/lib/gcc/$target/$gcc_version
gcc_target_lib=$gcce/$target/lib
include=$epocroot/epoc32/include
dso_dir=$epocroot/epoc32/release/armv5/lib
shims=$src/symbian-rs/shims

# The runtime members the shims need (experiment 109, section 1). Step 4 fails unless this
# is exactly what the link takes, so a change here is a change of what the package ships
# under GPL-3.0-or-later WITH GCC-exception-3.1.
supcxx_members=(del_ops.o eh_personality.o)
gcc_members=(pr-support.o _thumb1_case_uqi.o)

for tool in "$cxx" "$ar" "$ld" "$nm" "$objcopy"; do
  [ -x "$tool" ] || fail "$tool is missing: is $gcce the installed gcce;$gcc_version?"
done
[ "$("$cxx" -dumpversion)" = "$gcc_version" ] ||
  fail "$cxx is GCC $("$cxx" -dumpversion), not $gcc_version"
[ -f "$include/gcce/gcce.h" ] && [ -f "$dso_dir/euser.dso" ] ||
  fail "$epocroot is not an S60 SDK (no epoc32/include/gcce/gcce.h or epoc32/release/armv5/lib/euser.dso)"
[ -d "$shims/common" ] && [ -d "$shims/s60" ] || fail "$src has no symbian-rs/shims/{common,s60}"

# --- the compile line ------------------------------------------------------------------
# symdev's own options for its shims (RustBuild::SHIM_OPTIONS). The tag's driver must say
# the same, or the prebuilt objects would differ from what symdev compiles per application.
shim_options=(-ffunction-sections -fdata-sections -fno-rtti -Os)
driver=$src/crates/symdev-build/src/driver/rust_shims.rs
expected=$(printf '"%s",' "${shim_options[@]}")
declared=$(tr -d ' \n' <"$driver" 2>/dev/null |
  sed -n "s/.*SHIM_OPTIONS:\[&'staticstr;[0-9]*\]=\[\([^]]*\)\];.*/\1/p")
[ "$declared," = "$expected" ] || [ "$declared" = "$expected" ] ||
  fail "the tag's RustBuild::SHIM_OPTIONS (${declared:-not found in $driver}) is not this script's (${expected%,}); compile the shims as the tag's symdev does, then update shim_options"

"$pkgtools" sdk-casefold "$include" "$work/casefold" >/dev/null ||
  fail "cannot build the case-fold overlay of $include"
cxx_args=(
  -O2 -fexceptions -march=armv5t -mapcs
  "${shim_options[@]}"
  -mthumb-interwork -mthumb -msoft-float -fpermissive -Wno-narrowing
  -D__SYMBIAN32__ -D__EPOC32__ -D__MARM__ -D__GCCE__ -D__EXE__
  -include "$include/gcce/gcce.h"
  "-D__PRODUCT_INCLUDE__=\"$include/variant/symbian_os_v9.3.hrh\""
  -nostdinc -c
  -D__MARM_THUMB__ -D__MARM_INTERWORK__ -DNDEBUG -D_UNICODE -D__S60_3X__
  -D__SERIES60_3X__ -D__EABI__ -D__MARM_ARMV5__ -D__SUPPORT_CPP_EXCEPTIONS__
)

# --- 1. compile, 2. TARGET2 ------------------------------------------------------------
rm -rf "$work/obj" "$work/rt" "$work/closure" "$out/lib"
mkdir -p "$work/obj/common" "$work/obj/s60" "$work/rt" "$work/closure" "$out/lib"
for kind in common s60; do
  for source in "$shims/$kind"/*.cpp; do
    obj=$work/obj/$kind/$(basename "${source%.cpp}").o
    log=${obj%.o}.log
    # -I the source's own directory first, as symdev does, so `#include "symrs_shim.h"`
    # finds its neighbour; the SDK headers' warnings go to the log. On a failure only the
    # diagnostics' own lines are shown, not the source lines GCC quotes under them, which
    # may be the SDK's: CI logs can be public, the SDK may not be passed on.
    "$cxx" "${cxx_args[@]}" -I "$shims/$kind" -I "$include" -I "$include/variant" \
      -I "$work/casefold" -I "$gcc_lib/include" -o "$obj" "$source" 2>"$log" ||
      { grep -E '^(In file included from |[[:space:]]+from )|: (fatal error|error|warning|note): ' \
        "$log" >&2 || true; fail "cannot compile $source (full log: $log)"; }
  done
done
"$nm" "$work/obj/s60/symrs_avkon.o" | grep -qx ' *U symrs_uid3' ||
  fail "symrs_avkon.o does not take its UID3 from symrs_uid3: the tag's shim needs SYMRS_UID3 at compile time"
"$pkgtools" target2-abs32 "$work"/obj/common/*.o "$work"/obj/s60/*.o

# `ar crD`: no timestamps, owners or modes from this machine, so the same objects always
# make the same archive. Members in name order, as the shell globs them.
(cd "$work/obj/common" && "$ar" crD "$out/lib/libsymrs.a" ./*.o)
(cd "$work/obj/s60" && "$ar" crD "$out/lib/libsymrs_ui.a" ./*.o)

# --- 3. the runtime members ------------------------------------------------------------
runtime() { # <name> <GCCE archive> <members...>
  local name=$1 from=$2 dir=$work/rt/$1
  shift 2
  mkdir -p "$dir"
  (cd "$dir" && "$ar" x "$from" "$@") || fail "cannot take $* out of $from"
  for member in "$@"; do
    "$objcopy" --strip-debug "$dir/$member"
  done
  "$pkgtools" target2-abs32 "${@/#/$dir/}"
  (cd "$dir" && "$ar" crD "$out/lib/$name" "$@")
}
runtime libsupc++.a "$gcc_target_lib/libsupc++.a" "${supcxx_members[@]}"
runtime libgcc.a "$gcc_lib/libgcc.a" "${gcc_members[@]}"

# --- 4. closure ------------------------------------------------------------------------
# The import libraries of symdev's Rust link line, an `[ui]` one, in its order (experiment
# 109, "The link line symdev uses"): whatever an SDK DLL exports is taken from it, and only
# the rest from libsupc++/libgcc, which come last.
dsos=(euser.dso drtaeabi.dso dfpaeabi.dso dfprvct2_2.dso scppnwdl.dso drtrvct2_2.dso
  apparc.dso cone.dso eikcore.dso avkon.dso gdi.dso efsrv.dso bafl.dso esock.dso
  insock.dso eikcoctl.dso eikctl.dso)
closure_link() { # <name> <-L dirs for -lsupc++ -lgcc...>
  local name=$1
  shift
  "$ld" -shared -nostdlib --target1-abs -o "$work/closure/$name.elf" \
    -Map "$work/closure/$name.map" \
    --whole-archive "$out/lib/libsymrs.a" "$out/lib/libsymrs_ui.a" --no-whole-archive \
    -L "$dso_dir" "${dsos[@]/#/-l:}" "$@" -lsupc++ -lgcc ||
    fail "the closure link against $* failed"
  "$nm" -D --undefined-only "$work/closure/$name.elf" | sort >"$work/closure/$name.undefined"
}
closure_link gcce -L "$gcc_lib" -L "$gcc_target_lib"
shipped=()
for member in "${supcxx_members[@]}"; do shipped+=("libsupc++.a($member)"); done
for member in "${gcc_members[@]}"; do shipped+=("libgcc.a($member)"); done
"$pkgtools" runtime-closure "$work/closure/gcce.map" "${shipped[@]}" ||
  fail "the shims need other GCC runtime members than prebuilt.sh ships"
closure_link prebuilt -L "$out/lib"
diff -u "$work/closure/gcce.undefined" "$work/closure/prebuilt.undefined" ||
  fail "linked against lib/ alone the shims leave other symbols undefined than against GCCE"

# --- notices ---------------------------------------------------------------------------
cp "$tools/notices/gcc-$gcc_version/COPYING3" "$tools/notices/gcc-$gcc_version/COPYING.RUNTIME" \
  "$out/"
gcce_source=$(sed -n 's/^url = "\(.*gcc-[0-9.]*\.tar\.xz\)"$/\1/p' \
  "$here/../../gcce/$gcc_version/recipe.toml")
cat >"$out/NOTICE" <<EOF
prebuilt/ of the rust-sdk package: what symdev links into a Rust application, compiled once
so that the application can be linked without GCCE (symdev experiment 109).

lib/libsymrs.a, lib/libsymrs_ui.a
  symdev's C++ shims, symbian-rs/shims/common and symbian-rs/shims/s60 of this package,
  compiled by GCC $gcc_version (the gcce;$gcc_version package) against the S60 3rd Edition FP2
  SDK headers, with R_ARM_TARGET2 relocations rewritten to R_ARM_ABS32. MIT, like the rest
  of this package (LICENSE at its root).

lib/libsupc++.a: ${supcxx_members[*]}
lib/libgcc.a: ${gcc_members[*]}
  Members of GCC $gcc_version's libsupc++.a and libgcc.a, taken from the gcce;$gcc_version
  package, stripped of debug information (objcopy --strip-debug); nothing else changed.
  Licensed under the GNU General Public License, version 3 or later (COPYING3), with the
  GCC Runtime Library Exception, version 3.1 (COPYING.RUNTIME):
  GPL-3.0-or-later WITH GCC-exception-3.1. Their sources are in GCC $gcc_version
  ($gcce_source): libstdc++-v3/libsupc++/del_ops.cc and eh_personality.cc,
  libgcc/config/arm/pr-support.c and lib1funcs.S (L_thumb1_case_uqi).
  The Corresponding Source is the source archive of the gcce;$gcc_version package, published
  next to this package: the "source-code" of gcce;$gcc_version in the same bucket's
  index.toml (src/gcce/$gcc_version/<sha256>.tar.gz). It holds that GCC tarball and the
  recipe that built the gcce package from it.
EOF

ls -l "$out/lib"
(cd "$out" && sha256sum lib/*.a)
echo "prebuilt: $(cat "$out"/lib/*.a | wc -c) bytes in lib/"
