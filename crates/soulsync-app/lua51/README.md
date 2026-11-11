# Lua 5.1 for DeSmuME

DeSmuME on Windows needs `lua51.dll` next to its `.exe` to run Lua scripts and
does not ship it. SoulSync carries the DLL for 64-bit (`x64`) and 32-bit
(`x86`) DeSmuME and puts the matching one there when it is missing.

Both are plain builds of [Lua 5.1.5](https://www.lua.org/ftp/lua-5.1.5.tar.gz)
with MinGW-w64, without changes to the sources. To build them again, from the
`src` folder of the Lua sources:

```sh
for arch in x86_64 i686; do
  $arch-w64-mingw32-gcc -O2 -shared -DLUA_BUILD_AS_DLL -static-libgcc -s \
    -o lua51-$arch.dll $(ls *.c | grep -v -e '^lua.c$' -e '^luac.c$' -e '^print.c$')
done
```

## License

Lua is distributed under the MIT license:

> Copyright (C) 1994-2012 Lua.org, PUC-Rio. All rights reserved.
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to
> deal in the Software without restriction, including without limitation the
> rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
> sell copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in
> all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
> FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
> IN THE SOFTWARE.
