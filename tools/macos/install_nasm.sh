#!/bin/bash
# Build the verified NASM 2.x release only in the disposable GitHub runner directory.
# libaom 3.11's CMake feature check is not compatible with NASM 3.x help output.
set -euo pipefail
base="${RUNNER_TEMP:?GitHub runner only}/lcl-nasm"
mkdir -p "$base"
curl --fail --location --retry 3 'https://www.nasm.us/pub/nasm/releasebuilds/2.16.03/nasm-2.16.03.tar.xz' -o "$base/nasm.tar.xz"
echo '1412a1c760bbd05db026b6c0d1657affd6631cd0a63cddb6f73cc6d4aa616148  '"$base/nasm.tar.xz" | shasum -a 256 -c -
tar -xf "$base/nasm.tar.xz" -C "$base"
cd "$base/nasm-2.16.03"
./configure --prefix="$base/install"
make -j3
make install
"$base/install/bin/nasm" -v
echo "$base/install/bin" >> "${GITHUB_PATH:?}"
