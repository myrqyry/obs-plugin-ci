# Shared OBS plugin CI

A reusable native build matrix and verified test-artifact contract. Derived
from Streaming Media's successful OBS 32.2.0/33.0.3 matrix, including
Linux x86_64, Windows x64, macOS Intel and Apple Silicon.

## Contract (initial pilot)

The calling repository owns its source-specific verification drivers.
This first extraction standardizes runner selection, SDK versions,
read-only permissions, tests before upload, archive naming, integrity,
artifact retention and the absence of automatic releases. It does not
yet centralize SDK provisioning or plugin-specific dependency recipes.

Required caller files:

- `.github/scripts/prepare-obs32-sdk.sh SDK_WORK OBS_VERSION`
- `.github/scripts/verify-linux.sh SDK WORK MODULE MINIMUM_TESTS`
- `.github/scripts/verify-native.py WORK --obs-version OBS_VERSION`
- Existing native-verifier and README-link tests/checker.

Each verifier must build Release, execute its required native regression
suite, enforce a module-specific payload allowlist, validate installation
layout and archive integrity, and produce precisely:

    WORK/MODULE-TARGET-obsVERSION-test.tar.gz  (Linux)
    WORK/MODULE-TARGET-obsVERSION-test.zip     (Windows/macOS)
    WORK/SHA256SUMS

Targets: linux-x86_64, windows-x64, macos-arm64, macos-x86_64.
The SHA-256 file contains exactly one digest plus the archive filename.
No logs, credentials, scenes, SDKs, bundled runtime executables or raw
build directories are uploaded. Artifact retention is 14 days, and
artifacts are explicitly test packages, not release binaries. On a public
caller repository, assume artifacts are externally downloadable.

## Adoption

Streaming Media now calls the published workflow pinned at
`db6c0a2495456274ecf91f18fc83dd64dc6d29e8`. Its temporary local copy has
been removed. [Hosted pilot run](https://github.com/myrqyry/streaming-media-source/actions/runs/45558327793)
has started all eight jobs; upload results remain unqualified.

Example caller (requires the verifier contract above):

```yaml
jobs:
  build:
    uses: myrqyry/obs-plugin-ci/.github/workflows/build.yml@db6c0a2495456274ecf91f18fc83dd64dc6d29e8
    with:
      module: obs-streaming-media
      minimum-tests: 9
```

Do not migrate other repositories until their actual build targets,
resource files, dependency licenses, regression gates and package layouts
have been assessed. Preserve plugin-specific codec, GPU, inference and
signing qualification separately. A green build is not runtime proof.

Run local contract tests with Python 3.11+, Bash and PyYAML:

    PYTHONDONTWRITEBYTECODE=1 python3 tests/test_workflow.py

The tests cover the eight unique target/version combinations, exact
upload paths, permission boundaries and real checksum-gate execution.
Hosted workflow-call execution and artifact upload remain to be verified.

Source: myrqyry/streaming-media-source, merged baseline 2fbb807.
