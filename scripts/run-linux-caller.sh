#!/usr/bin/env bash
# Caller-owned native build/test/staging, shared SDK and package mechanics.
set -euo pipefail
engine="$(cd "$(dirname "$0")/.." && pwd)"
CI_ROOT="$(realpath "$1")"
hook="$CI_ROOT/.github/scripts/ci-project.sh"
[[ "$TARGET" == linux-x86_64 && "$MODULE" =~ ^[A-Za-z0-9][A-Za-z0-9_-]*$ && "$PUBLIC_NAME" =~ ^[A-Za-z0-9][A-Za-z0-9_-]*$ ]]
[[ "$ARCHIVE" == "$MODULE-linux-x86_64-obs$OBS_VERSION-test.tar.gz" ]]
[[ "$MINIMUM_TESTS" =~ ^[1-9][0-9]*$ ]]
case "$OBS_VERSION" in
  32.2.0) CI_LAYOUT=legacy; CI_OBS_MAJOR=32; CI_OBS_MINOR=2 ;;
  33.0.3) CI_LAYOUT=modern; CI_OBS_MAJOR=33; CI_OBS_MINOR=0 ;;
  *) echo "Unsupported SDK" >&2; exit 2 ;;
esac
CI_WORK="$RUNNER_TEMP/caller-work"
mode="${2:-build}"
cache="${OBS_CI_LINUX_SDK_CACHE:-false}"
[[ "$mode" == build || "$mode" == --sdk-cache-key || "$mode" == --sdk-only ]]
[[ "$cache" == true || "$cache" == false ]]
if [[ "$mode" == --sdk-only ]]; then
    [[ "$cache" == true ]]
fi
if [[ "$mode" == --sdk-cache-key ]]; then
    [[ "$cache" == true && "${ENGINE_REVISION:-}" =~ ^[a-f0-9]{40}$ && -n "${GITHUB_OUTPUT:-}" ]]
fi
if [[ "$cache" == true ]]; then
    [[ -z "${OBS_CI_LOCAL_SDK:-}" && -z "${OBS_CI_LOCAL_BUILD:-}" ]]
fi
if [[ "$cache" == true && "$mode" != --sdk-cache-key ]]; then
    test -f "$CI_WORK/sdk-fingerprint.json"
else
    mkdir "$CI_WORK"
fi
CI_BUILD="$CI_WORK/build"
CI_STAGE="$CI_WORK/stage"
CI_SDK="$CI_WORK/obs/sdk"
CI_OBSCI="$engine/target/release/obs-ci"
test -x "$CI_OBSCI"
test -f "$hook"
verify_ctest() {
    xvfb-run -a ctest --test-dir "$CI_BUILD" --output-on-failure --no-tests=error --output-junit "$CI_WORK/ctest.xml"
    python3 - "$CI_WORK/ctest.xml" "$MINIMUM_TESTS" <<'PYTEST'
import sys, xml.etree.ElementTree as E
cases=E.parse(sys.argv[1]).findall(".//testcase")
assert len(cases)>=int(sys.argv[2]), "Too few registered tests"
assert all(c.find(k) is None for c in cases for k in ("failure","error","skipped"))
assert not any(c.get("status") in ("notrun","disabled") for c in cases)
print(f"Verified {len(cases)} required CTest cases")
PYTEST
}
source "$hook"
if [[ "$cache" == true ]]; then
    case "$OBS_VERSION" in
        32.2.0) recipe=legacy-core ;;
        33.0.3) recipe=streaming-minimal ;;
    esac
    sdk_work="$CI_WORK/obs"
    fingerprint_args=(--obs-version "$OBS_VERSION" --recipe "$recipe" --work "$sdk_work")
    if [[ "$mode" == --sdk-cache-key ]]; then
        ci_dependencies
        "$CI_OBSCI" sdk-fingerprint "${fingerprint_args[@]}" > "$CI_WORK/sdk-fingerprint.json"
        key=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["cache_key"])' < "$CI_WORK/sdk-fingerprint.json")
        [[ "$key" =~ ^[a-f0-9]{64}$ ]]
        printf 'key=obs-ci-sdk-v1-%s-%s\n' "$ENGINE_REVISION" "$key" >> "$GITHUB_OUTPUT"
        exit 0
    fi
    # The dependencies were installed before computing the cache key. Do not
    # mutate the environment between fingerprint validation and SDK reuse.
    current=$("$CI_OBSCI" sdk-fingerprint "${fingerprint_args[@]}")
    [[ "$current" == "$(cat "$CI_WORK/sdk-fingerprint.json")" ]]
    if [[ -e "$sdk_work" ]]; then
        echo "Verifying restored SDK and exact environment"
    else
        echo "Building pinned SDK (no exact cache hit)"
    fi
    if ! "$CI_OBSCI" sdk-prepare "${fingerprint_args[@]}" \
        --cache "$RUNNER_TEMP/obs-ci-sdk-sources" --jobs 4 > "$CI_WORK/sdk-receipt.json"; then
        if [[ -f "$sdk_work/sdk-build.log" ]]; then tail -n 200 "$sdk_work/sdk-build.log" >&2; fi
        exit 1
    fi
    [[ "$(cat "$CI_SDK/OBS_SDK_VERSION")" == "$OBS_VERSION" ]]
elif [[ -n "${OBS_CI_LOCAL_SDK:-}" ]]; then
    [[ "${GITHUB_ACTIONS:-}" != true ]]
    CI_SDK="$(realpath "$OBS_CI_LOCAL_SDK")"
    echo "Local SDK only: $CI_SDK"
else
    ci_dependencies
    bash "$engine/scripts/prepare-linux-sdk.sh" "$CI_WORK/obs" "$OBS_VERSION"
    [[ "$(cat "$CI_SDK/OBS_SDK_VERSION")" == "$OBS_VERSION" ]]
fi
test -f "$CI_SDK/lib/cmake/libobs/libobsConfig.cmake"
grep -Eq "^#define LIBOBS_API_MAJOR_VER $CI_OBS_MAJOR$" "$CI_SDK/include/obs/obs-config.h"
grep -Eq "^#define LIBOBS_API_MINOR_VER $CI_OBS_MINOR$" "$CI_SDK/include/obs/obs-config.h"
# Specialized callers can retain their complete existing build/test/package
# verifier. Only the pinned SDK provisioning is shared in this mode.
if [[ "$mode" == --sdk-only ]]; then
    exit 0
fi
if [[ -n "${OBS_CI_LOCAL_BUILD:-}" ]]; then
    [[ "${GITHUB_ACTIONS:-}" != true && -n "${OBS_CI_LOCAL_SDK:-}" ]]
    CI_BUILD="$(realpath "$OBS_CI_LOCAL_BUILD")"
    grep -Fx "CMAKE_HOME_DIRECTORY:INTERNAL=$CI_ROOT" "$CI_BUILD/CMakeCache.txt"
    echo "Using existing local build tree (not a clean hosted build)"
    cmake --build "$CI_BUILD" --parallel 2
else
    ci_build
fi
mkdir "$CI_WORK/home" "$CI_STAGE"
export HOME="$CI_WORK/home" XDG_CONFIG_HOME="$CI_WORK/home/config" XDG_DATA_HOME="$CI_WORK/home/data" XDG_CACHE_HOME="$CI_WORK/home/cache"
export LD_LIBRARY_PATH="$CI_SDK/lib:$CI_SDK/lib/x86_64-linux-gnu"
export PKG_CONFIG_PATH="$CI_SDK/lib/pkgconfig:$CI_SDK/lib/x86_64-linux-gnu/pkgconfig"
export LIBGL_ALWAYS_SOFTWARE=1 PYTHONDONTWRITEBYTECODE=1
# Caller preserves its meaningful runtime checks and creates the exact staged tree.
ci_test
ci_stage
module="$CI_STAGE/$MODULE/$MODULE.so"
if [[ "$CI_LAYOUT" == legacy ]]; then module="$CI_STAGE/$MODULE/bin/64bit/$MODULE.so"; fi
test -s "$module"
ldd "$module" > "$CI_WORK/dependencies.txt"
! grep -q 'not found' "$CI_WORK/dependencies.txt"
"$CI_OBSCI" pack --stage "$CI_STAGE" --output "$RUNNER_TEMP/plugin-ci" --archive-name "$ARCHIVE"
"$CI_OBSCI" inspect-module "$RUNNER_TEMP/plugin-ci/$ARCHIVE" --module "$MODULE" --target "$TARGET" --obs-version "$OBS_VERSION"
