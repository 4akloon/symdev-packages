#!/usr/bin/env bash
# Build gcce;12.1.0 (binutils 2.29.1 + GCC 12.1.0 for arm-none-symbianelf) into one prefix.
#
#   build.sh <prefix>
#
# Runs in the current directory: fetches every [[source]] of recipe.toml that is not
# there yet, checks all SHA-256s, unpacks and builds. <prefix> must be absolute and is
# only the install location — the installed tree is relocatable.
#
# Steps and flags are the ones recovered from the host build symdev was developed with
# (GCC4Symbian's build-toolchain.sh, plus binutils 2.29.1 configured like its binutils
# step); symdev experiment 107. Nothing here was added beyond what that build did, except
# that binutils is 2.29.1 (GCC4Symbian's 2.35 ld rejects the SDK's euser.dso), gdb is not
# built, and what that build took from GCC4Symbian is ours (symdev experiment 108: the
# same target libraries and c++config.h): the two sys-include headers next to this script
# and a -D for libgcov instead of a changed libgcov-driver.c.
set -euo pipefail

if [ $# -ne 1 ] || [ "${1#/}" = "$1" ]; then
  echo "usage: build.sh <absolute prefix>" >&2
  exit 2
fi
prefix=$1
target=arm-none-symbianelf
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
jobs=$(nproc)

# The target libraries are built by the new cross compiler; a host's include or library
# search path must not reach it.
unset CFLAGS CXXFLAGS CPPFLAGS LDFLAGS CPATH C_INCLUDE_PATH CPLUS_INCLUDE_PATH \
  LIBRARY_PATH GCC_EXEC_PREFIX

# --- sources ---------------------------------------------------------------------------
fetch() {
  if command -v curl >/dev/null; then curl -fL --retry 3 -o "$2" "$1"
  else wget -O "$2" "$1"; fi
}
url=
: > SHA256SUMS
while IFS= read -r line; do
  case $line in
    'url = "'*) url=${line#url = \"}; url=${url%\"} ;;
    'sha256 = "'*)
      sha=${line#sha256 = \"}; sha=${sha%\"}
      file=${url##*/}
      [ -f "$file" ] || fetch "$url" "$file"
      echo "$sha  $file" >> SHA256SUMS
      url= ;;
  esac
done < "$here/recipe.toml"
sha256sum -c SHA256SUMS

rm -rf binutils-2.29.1 gcc-12.1.0 build-binutils build-gcc
tar -xf binutils-2.29.1.tar.xz
tar -xf gcc-12.1.0.tar.xz
for lib in gmp-6.1.0.tar.bz2 mpfr-4.1.0.tar.bz2 mpc-1.2.1.tar.gz isl-0.16.1.tar.bz2; do
  dir=${lib%.tar.*}
  rm -rf "$dir"
  tar -xf "$lib"
  mv "$dir" "gcc-12.1.0/${dir%-*}"
done

# --- binutils --------------------------------------------------------------------------
mkdir build-binutils
(
  cd build-binutils
  export CFLAGS="-pipe -Bstatic"
  ../binutils-2.29.1/./configure --target=$target --prefix="$prefix" \
    --disable-option-checking --enable-ld --enable-gold --enable-lto --enable-vtable-verify \
    --enable-werror=no --without-headers --disable-nls --disable-shared \
    --disable-libquadmath --enable-plugins --enable-multilib
  make -j"$jobs"
  make install-strip
)

# --- the target's sys-include ----------------------------------------------------------
# The target has no C library here; GCC's build only needs these two to exist (see each).
mkdir -p "$prefix/$target/sys-include"
cp "$here/sys-include/stdint.h" "$here/sys-include/stdio.h" "$prefix/$target/sys-include/"

# --- gcc -------------------------------------------------------------------------------
mkdir build-gcc
(
  cd build-gcc
  export CFLAGS="-pipe"
  # arm-none-symbianelf predefines no __INTPTR_TYPE__ (config.gcc gives it no *-stdint.h),
  # and libgcc/libgcov-driver.c casts a gcov_type to a pointer through it. Define it as
  # int, the type GCC's newlib-stdint.h gives intptr_t on ARM, for the target libraries
  # only; "-g -O2" is configure's default for a cross compiler. Only libgcov reads it and
  # DWARF does not record -D, so GCC's source stays unchanged (symdev experiment 108).
  export CFLAGS_FOR_TARGET="-g -O2 -D__INTPTR_TYPE__=int"
  ../gcc-12.1.0/./configure --target=$target --prefix="$prefix" --without-headers \
    --enable-languages="c,c++,lto" --enable-lto --enable-interwork \
    --enable-long-long --enable-tls --enable-multilib --enable-wchar_t \
    --enable-c99 --with-newlib --with-dwarf2 --with-static-standard-libraries \
    --disable-hosted-libstdcxx --disable-libstdcxx-pch --disable-shared \
    --disable-option-checking --disable-threads --disable-nls \
    --disable-win32-registry --disable-libssp --disable-libquadmath
  make -j"$jobs"
  make install-strip
)
