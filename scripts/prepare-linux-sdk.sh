#!/usr/bin/env bash
# Reused from obs-ui-augment at 4530772; pinned OBS 32.2.0 SDK recipe.
set -euo pipefail
work="$(realpath -m "$1")"
version="${2:-32.2.0}"
case "$version" in
  32.2.0) sha=c333e4a7d5c4a94c7bb4833f046368e0ac5e981fb3964ca4981b23be1dfbda4a; extra=() ;;
  33.0.3) sha=b0eff217a972e1dd9951ce24d66595c723dde847461b16de07eb33d75ec34659; extra=(-DENABLE_SCRIPTING=OFF -DENABLE_BROWSER=OFF) ;;
  *) echo "Unsupported OBS SDK: $version" >&2; exit 1 ;;
esac
# A side-by-side FFmpeg 8 development install for older Linux desktops.
# Never apply a local override to a hosted qualification or OBS32.
if [[ -n "${OBS_CI_LOCAL_FFMPEG_PREFIX:-}" ]]; then
  if [[ "${GITHUB_ACTIONS:-}" == true || "$version" != 33.0.3 ]]; then
    echo "FFmpeg override is only supported for local OBS33" >&2
    exit 2
  fi
  if [[ "$OBS_CI_LOCAL_FFMPEG_PREFIX" != /* || ! -d "$OBS_CI_LOCAL_FFMPEG_PREFIX/lib/pkgconfig" ]]; then
    echo "Invalid local FFmpeg prefix" >&2
    exit 2
  fi
  ffmpeg="$(realpath "$OBS_CI_LOCAL_FFMPEG_PREFIX")"
  export PKG_CONFIG_PATH="$ffmpeg/lib/pkgconfig"
  # Verify all required components resolve to the same prefix and FFmpeg 8 ABI.
  for spec in avcodec:62 avformat:62 avutil:60 swscale:9 swresample:6; do
    component="${spec%:*}"
    major="${spec#*:}"
    if [[ "$(pkg-config --variable=prefix "lib$component")" != "$ffmpeg" ||
          "$(pkg-config --modversion "lib$component")" != "$major".* ||
          ! -s "$ffmpeg/lib/lib$component.so" ]]; then
      echo "Invalid local FFmpeg component: lib$component (expected major $major at $ffmpeg)" >&2
      exit 2
    fi
  done
  export LD_LIBRARY_PATH="$ffmpeg/lib"
  extra+=("-DCMAKE_PREFIX_PATH=$ffmpeg")
  echo "Using isolated local FFmpeg 8 development prefix: $ffmpeg"
fi
sdk="$work/sdk"
archive="$work/obs-studio-$version.tar.gz"
mkdir -p "$work"
curl -fsSL --retry 5 "https://github.com/obsproject/obs-studio/archive/refs/tags/$version.tar.gz" -o "$archive"
printf '%s  %s\n' "$sha" "$archive" | sha256sum -c -
tar -xzf "$archive" -C "$work"
cmake -S "$work/obs-studio-$version" -B "$work/obs-build" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$sdk" \
  -DOBS_CMAKE_VERSION:STRING=3.0.0 -DOBS_VERSION_OVERRIDE:STRING="$version" \
  -DENABLE_PLUGINS=OFF -DENABLE_FRONTEND=OFF "${extra[@]}"
cmake --build "$work/obs-build" --target obs-frontend-api libobs-opengl --parallel 4
cmake --install "$work/obs-build" --component Development --config Release --prefix "$sdk"
cmake --install "$work/obs-build" --component Runtime --config Release --prefix "$sdk"
test -s "$sdk/lib/cmake/libobs/libobsConfig.cmake"
test -s "$sdk/lib/cmake/obs-frontend-api/obs-frontend-apiConfig.cmake"
printf '%s\n' "$version" > "$sdk/OBS_SDK_VERSION"
