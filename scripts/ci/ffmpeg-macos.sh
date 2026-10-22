#!/usr/bin/env bash
# Builds the FFmpeg a macOS bundle ships, for MACOSX_DEPLOYMENT_TARGET.
#
#   scripts/ci/ffmpeg-macos.sh PREFIX
#
# Homebrew's FFmpeg is built for the runner's own macOS, and so are the
# thirty-odd libraries it brings (vpx, aom, rav1e, theora, X11...): a
# bundle made from it needs that macOS too, and dyld refuses the whole app
# at launch on anything older (Concat #342, #372). This builds FFmpeg with
# what Montora uses and nothing else, every piece for the deployment
# target:
#
# - x264, x265 (8- and 10-bit in one library) and SVT-AV1, the software
#   encoders encode.rs falls back on;
# - dav1d, the AV1 decoder;
# - VideoToolbox and AudioToolbox, the system's hardware codecs.
#
# The four libraries are linked into FFmpeg's own dylibs statically, so
# the bundle carries FFmpeg's six libraries and nothing more. No
# libavdevice (see concat-media/Cargo.toml) and no programs.
#
# The sources are the ones scripts/ci/deps.lock pins, checked by digest.
# x264 and x265 are GPL, so FFmpeg is configured --enable-gpl, as the
# Windows and Linux builds from BtbN are; Montora is AGPL-3.0-or-later,
# which takes GPL code.
#
# Needs: cmake, meson, ninja, nasm (on Intel), pkg-config. Runs on the
# architecture it builds for.
set -euo pipefail

prefix=${1:?usage: $0 PREFIX}
: "${MACOSX_DEPLOYMENT_TARGET:?the deployment target to build for}"
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
jobs=$(sysctl -n hw.ncpu)
mkdir -p "$prefix"
prefix=$(cd "$prefix" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

export CFLAGS="-mmacosx-version-min=$MACOSX_DEPLOYMENT_TARGET -O2"
export CXXFLAGS="$CFLAGS"
export LDFLAGS="-mmacosx-version-min=$MACOSX_DEPLOYMENT_TARGET"
export PKG_CONFIG_PATH="$prefix/lib/pkgconfig"
# x265 and SVT-AV1 still name CMake versions CMake 4 no longer accepts.
cmake_common=(
  -DCMAKE_BUILD_TYPE=Release
  -DCMAKE_INSTALL_PREFIX="$prefix"
  -DCMAKE_OSX_DEPLOYMENT_TARGET="$MACOSX_DEPLOYMENT_TARGET"
  -DCMAKE_POLICY_VERSION_MINIMUM=3.5
  -DBUILD_SHARED_LIBS=OFF
)

# Unpacks the pinned source NAME into $work/NAME and prints that folder.
source_of() {
  local name=$1
  "$here/fetch.sh" "src-$name" "$work/$name.archive" >&2
  mkdir -p "$work/$name"
  tar -xf "$work/$name.archive" -C "$work/$name" --strip-components=1
  echo "$work/$name"
}

echo "::group::dav1d"
src=$(source_of dav1d)
meson setup "$src/build" "$src" --prefix="$prefix" --libdir=lib \
  --buildtype=release --default-library=static \
  -Denable_tools=false -Denable_tests=false
ninja -C "$src/build" install
echo "::endgroup::"

echo "::group::x264"
src=$(source_of x264)
(cd "$src" && ./configure --prefix="$prefix" --enable-static --enable-pic \
  --disable-cli --extra-cflags="$CFLAGS" --extra-ldflags="$LDFLAGS" \
  && make -j"$jobs" && make install)
echo "::endgroup::"

# 10-bit first, built without its C API, then linked into the 8-bit
# library, the way x265's own multilib build does: one libx265 that encodes
# both, so a 10-bit export (Main10) does not need a second library.
echo "::group::x265"
src=$(source_of x265)
cmake -S "$src/source" -B "$src/b10" -G Ninja "${cmake_common[@]}" \
  -DHIGH_BIT_DEPTH=ON -DEXPORT_C_API=OFF -DENABLE_SHARED=OFF -DENABLE_CLI=OFF
ninja -C "$src/b10"
cp "$src/b10/libx265.a" "$src/libx265_main10.a"
cmake -S "$src/source" -B "$src/b8" -G Ninja "${cmake_common[@]}" \
  -DENABLE_SHARED=OFF -DENABLE_CLI=OFF -DLINKED_10BIT=ON \
  -DEXTRA_LIB="$src/libx265_main10.a"
ninja -C "$src/b8" install
# Installed first, then replaced: left in the build folder, the merged
# library could be rebuilt over by the install.
libtool -static -o "$prefix/lib/libx265.a" "$src/b8/libx265.a" "$src/libx265_main10.a"
echo "::endgroup::"

echo "::group::SVT-AV1"
src=$(source_of svtav1)
cmake -S "$src" -B "$src/build" -G Ninja "${cmake_common[@]}" \
  -DBUILD_APPS=OFF -DBUILD_DEC=OFF -DBUILD_TESTING=OFF
ninja -C "$src/build" install
echo "::endgroup::"

# --disable-autodetect: nothing is picked up because the runner happens to
# have it (Homebrew's xz, X11, SDL...). What is wanted is named. iconv, for
# subtitles in other encodings, is the system's libiconv, which nothing
# links by name once detection is off: named here too.
echo "::group::FFmpeg"
src=$(source_of ffmpeg)
(cd "$src" && ./configure --prefix="$prefix" \
  --enable-shared --disable-static \
  --disable-programs --disable-doc --disable-avdevice \
  --enable-gpl --disable-autodetect \
  --enable-zlib --enable-bzlib --enable-iconv \
  --enable-videotoolbox --enable-audiotoolbox \
  --enable-libdav1d --enable-libx264 --enable-libx265 --enable-libsvtav1 \
  --pkg-config-flags=--static --extra-libs=-liconv \
  --extra-cflags="$CFLAGS" --extra-cxxflags="$CXXFLAGS" --extra-ldflags="$LDFLAGS" \
  && make -j"$jobs" && make install)
echo "::endgroup::"

# What the build is for: every dylib names the deployment target, and none
# loads anything outside the system but its own siblings.
status=0
for lib in "$prefix"/lib/lib*.dylib; do
  [ -L "$lib" ] && continue
  minos=$(otool -l "$lib" | awk '/LC_BUILD_VERSION/ { b = 1 } b && $1 == "minos" { print $2; exit }')
  echo "$(basename "$lib"): macOS $minos"
  if [ "$minos" != "$MACOSX_DEPLOYMENT_TARGET" ]; then
    echo "  built for macOS $minos, not $MACOSX_DEPLOYMENT_TARGET" >&2
    status=1
  fi
  if otool -L "$lib" | tail -n +2 | grep -v -E "^[[:space:]]+(/usr/lib/|/System/|$prefix/lib/)"; then
    echo "  loads a library from outside the system" >&2
    status=1
  fi
done
exit $status
