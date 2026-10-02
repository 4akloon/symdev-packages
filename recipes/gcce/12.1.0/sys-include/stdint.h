/* <stdint.h> for building GCC 12.1.0 for arm-none-symbianelf (symdev's gcce;12.1.0).

   Written for symdev from what GCC's own sources and configure scripts require of this
   file; not copied or adapted from any other <stdint.h>.

   This toolchain has no C library: Symbian code is compiled with -nostdinc against the
   SDK's headers, so nothing but GCC's build reads this file.  build.sh puts it in
   <prefix>/arm-none-symbianelf/sys-include before GCC is configured, because:

   - libsupc++/new_opa.cc includes <stdint.h> and, when the target has no aligned
     allocator (newlib's memalign is assumed here), uses uintptr_t;
   - libstdc++'s configure checks for the header and records _GLIBCXX_HAVE_STDINT_H.

   It declares only the two pointer-sized types, as GCC's config/newlib-stdint.h does for
   newlib targets: intptr_t has ptrdiff_t's type, uintptr_t has size_t's.  It is not a
   complete C99 <stdint.h> on purpose: with every exact, least, fast and max type and
   their limit macros, libstdc++'s configure would also define
   _GLIBCXX_USE_C99_STDINT_TR1, which the toolchain symdev was developed with lacks.

   SPDX-License-Identifier: MIT

   Copyright (c) 2026 The symdev authors

   Permission is hereby granted, free of charge, to any person obtaining a copy of this
   software and associated documentation files (the "Software"), to deal in the Software
   without restriction, including without limitation the rights to use, copy, modify,
   merge, publish, distribute, sublicense, and/or sell copies of the Software, and to
   permit persons to whom the Software is furnished to do so, subject to the following
   conditions:

   The above copyright notice and this permission notice shall be included in all copies
   or substantial portions of the Software.

   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
   INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
   PARTICULAR PURPOSE AND NONINFRINGEMENT.  IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
   HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF
   CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE
   OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.  */

#ifndef _SYMDEV_GCCE_STDINT_H
#define _SYMDEV_GCCE_STDINT_H

typedef __PTRDIFF_TYPE__ intptr_t;
typedef __SIZE_TYPE__ uintptr_t;

#endif
