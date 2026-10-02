/* <stdio.h> for building GCC 12.1.0 for arm-none-symbianelf (symdev's gcce;12.1.0).

   Written for symdev from what GCC's own sources and configure scripts require of this
   file; not copied or adapted from any other <stdio.h>.

   This toolchain has no C library: Symbian code is compiled with -nostdinc against the
   SDK's headers, so nothing but GCC's build reads this file.  build.sh puts it in
   <prefix>/arm-none-symbianelf/sys-include before GCC is configured, because autoconf's
   default includes begin with <stdio.h>: without it every header check in libstdc++'s
   configure fails, and the installed c++config.h loses _GLIBCXX_HAVE_FLOAT_H,
   _GLIBCXX_HAVE_STDALIGN_H, _GLIBCXX_HAVE_STDBOOL_H and _GLIBCXX_HAVE_STDINT_H.

   No source GCC compiles for this target uses a declaration from it, so it declares
   nothing.  Declaring tmpnam, gets or C99's formatted input and output functions would
   make libstdc++'s configure enable features that need a C library this toolchain does
   not have.

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

#ifndef _SYMDEV_GCCE_STDIO_H
#define _SYMDEV_GCCE_STDIO_H

#endif
