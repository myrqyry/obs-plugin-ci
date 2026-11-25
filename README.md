# Shared OBS plugin CI

A reusable native build matrix and verified test-artifact contract. Derived
from Streaming Media's successful OBS 32.2.0/33.0.3 matrix, including
Linux x86_64, Windows x64, macOS Intel and Apple Silicon.

## Build contract

The calling repository owns its source-specific verification drivers.
This first extraction standardizes runner selection, SDK versions,
read-only permissions, tests before upload, archive naming, integrity,
artifact retention and the absence of automatic releases. It does not
yet centralize SDK provisioning or plugin-specific dependency recipes.

Default native driver caller files:

- `.github/scripts/prepare-obs32-sdk.sh SDK_WORK OBS_VERSION`
- `.github/scripts/verify-linux.sh SDK WORK MODULE MINIMUM_TESTS`
- `.github/scripts/verify-native.py WORK --obs-version OBS_VERSION`
- Existing native-verifier and README-link tests/checker.

Each verifier must build a release-mode configuration, execute its required native regression
suite, enforce a module-specific payload allowlist, validate installation
layout and archive integrity, and produce precisely:

    WORK/MODULE-TARGET-obsVERSION-test.tar.gz  (Linux)
    WORK/MODULE-TARGET-obsVERSION-test.zip     (Windows/macOS)
    WORK/SHA256SUMS

Targets: linux-x86_64, windows-x64, macos-arm64, macos-x86_64.
The SHA-256 file contains exactly one digest plus the archive filename.
The optional `package-name` input controls public archive and artifact names
independently of the installed module identity. The verified archive is renamed
without changing its bytes, and the checksum manifest is updated to match.
Omitting the input preserves the module-based name. Case-only public renames
use a temporary intermediate path to preserve exact spelling on
case-insensitive filesystems; distinct destination entries are rejected.

The shared binary handoff independently validates ZIP member CRCs or
tar/gzip readability and gzip checksums without extracting anything.
It rejects empty payloads, unsafe paths, links, special members,
duplicate or case-colliding paths, file/directory conflicts, and .git,
.env and .pdb payloads. Exact OBS layout, module/resource allowlists and
license requirements remain caller-owned.
No logs, credentials, scenes, SDKs, bundled runtime executables or raw
build directories are uploaded. Artifact retention is 14 days, and
artifacts are explicitly test packages, not release binaries. On a public
caller repository, assume artifacts are externally downloadable.

## Caller-owned drivers and qualified platform subsets

The default profile is `full` (eight target/version combinations).
`profile: linux-obs32` selects only the established Ubuntu 24.04 / OBS 32.2.0
target; unsupported profiles and invalid module/package identifiers or Linux test floors
fail at planning before native runners are allocated.
The default driver preserves the original pinned SDK verifier contract.
With `driver: caller`, the shared workflow runs the caller's
`.github/scripts/ci-verify.sh` after installing the common Linux dependencies.
It receives TARGET, MODULE, PUBLIC_NAME, OBS_VERSION, MINIMUM_TESTS, ARCHIVE and
RUNNER_TEMP. The caller owns SDK provisioning, any additional dependencies,
all regression and package-content gates, and must produce the archive and
single-entry checksum at RUNNER_TEMP/plugin-ci. A successful driver exit
does not replace the common archive checksum verification.

Optional `source-package: true` is available for caller-owned drivers.
On the Linux OBS 32.2.0 job, the caller must additionally prepare
RUNNER_TEMP/plugin-ci-source with PUBLIC_NAME-source.tar.gz, its SHA256SUMS
and the verified installed-file PAYLOAD-SHA256SUMS manifest. The source
archive checksum and nonempty, unique, relative payload-manifest paths are
checked before either artifact is uploaded. Source and binary
are uploaded separately with exact paths and 14-day retention. The shared source gate additionally checks a readable single-root tar/gzip\ncontainer, safe regular-file/directory paths, a root LICENSE or COPYING,\nand excludes common compiled-binary suffixes. Full license compliance,\ntracked-file allowlists and installed-payload hash validation remain\nthe caller's responsibility. No release is created.

## Adoption

Streaming Media now calls the published workflow pinned at
`197937941f13cd2adabd302a19691b44b62b2032`. Its temporary local copy has
been removed. [Hosted pilot run](https://github.com/myrqyry/streaming-media-source/actions/runs/45566510651)
completed all eight jobs; all eight public-name packages were downloaded,
inspected and checked against their SHA-256 manifests.

Example caller (requires the verifier contract above):

```yaml
jobs:
  build:
    uses: myrqyry/obs-plugin-ci/.github/workflows/build.yml@197937941f13cd2adabd302a19691b44b62b2032
    with:
      module: obs-streaming-media
      package-name: streaming-media-source
      minimum-tests: 9
```

Do not migrate other repositories until their actual build targets,
resource files, dependency licenses, regression gates and package layouts
have been assessed. Preserve plugin-specific codec, GPU, inference and
signing qualification separately. A green build is not runtime proof.

Run local contract tests with Python 3.11+, Bash and PyYAML:

    PYTHONDONTWRITEBYTECODE=1 python3 tests/test_workflow.py

The repository also runs this contract suite on Ubuntu, Windows, macOS\nApple Silicon and macOS Intel via its own ci.yml on main pushes, pull\nrequests and manual dispatches. This is infrastructure-only testing, not\nOBS SDK build or runtime qualification.\n\nThe tests cover the eight unique target/version combinations, exact
upload paths, permission boundaries and real checksum-gate execution.
Hosted workflow-call execution and artifact upload passed for the Streaming
Media pilot. The caller-driver/subset path also passed the [Framerot Linux OBS 32 pilot](https://github.com/myrqyry/framerot/actions/runs/45577168933), including all 18 native tests and both downloaded artifact audits.

Source: myrqyry/streaming-media-source, merged baseline 2fbb807.

## Rust CLI (local tooling)

The standalone obs-ci executable is an incremental port of the shared
CI contract. Build it locally with Rust 1.93.1 (the toolchain tested here):

    cargo build --release --locked
    ./target/release/obs-ci plan --module obs-streaming-media --minimum-tests 9
    ./target/release/obs-ci audit /path/to/module-test.zip

On Windows use target/release/obs-ci.exe. The plan command accepts the
current full and linux-obs32 profiles, driver and source-package options,
and optional comma-separated target and pinned OBS-version filters:

    ./target/release/obs-ci plan --module framerot --minimum-tests 18 --driver caller --source-package --targets linux-x86_64,windows-x64 --obs-versions 33.0.3

Matrix output retains the existing row fields and adds emit_source. Exactly
one selected Linux row hosts a requested source package, preferring the first
row in catalog order. Empty or unsupported selections and source requests
without Linux are rejected. Selection is a planning capability, not proof
that a particular plugin has been qualified on those targets. Audit is read-only:
it checks the adjacent SHA256SUMS, ZIP or tar/gzip integrity, safe member
paths and types, duplicate/case-colliding names, and basic payload exclusions.
It prints a JSON receipt with archive name, SHA-256, size and file counts.
For source packages:

    ./target/release/obs-ci audit-source /path/to/plugin-source.tar.gz --binary /path/to/plugin-test.tar.gz

The source gate checks a single root, nonempty root license, safe member
paths/types, common compiled-file exclusions and the adjacent
PAYLOAD-SHA256SUMS. Leading comment-only Git PAX metadata is accepted;
global path/size overrides are rejected. With --binary, the auditor
independently checks the binary archive and matches every regular-file
path and SHA-256 against the payload manifest, rejecting missing, extra
or changed files. Without --binary, payload_verified is false; only the
source archive and manifest structure have been checked. This is not a
full license-compliance or tracked-source provenance audit.

Run the Rust tests and the cross-implementation parity suite:

    cargo test --locked
    cargo build --locked
    python3 tests/test_rust_parity.py
    cargo clippy --locked --all-targets -- -D warnings

The existing Python workflow remains authoritative; the Rust tool does
not yet implement SDK
provisioning, caller builds, or publication. No caller needs Rust today.
Self-CI checks both implementations on all four runner families; hosted
qualification remains pending. The dependency lockfile is committed.

### Verified handoff and native inspection

    ./target/release/obs-ci handoff /path/to/module-test.zip --public-name public-test.zip
    ./target/release/obs-ci inspect-module /path/to/plugin-test.tar.gz --module framerot --target linux-x86_64 --obs-version 32.2.0

Handoff requires an audited binary archive and a matching SHA256SUMS in an
isolated staging directory on a filesystem supporting hard links. It
refuses destination collisions, preserves archive bytes, uses a staged
backup and atomically replaces the checksum manifest. On manifest failure
it attempts rollback; if restoration fails the backup path is reported.
It is not a crash-atomic multi-file transaction; do not run concurrent
writers against the same staging directory.

Native inspection checks the exact qualified OBS layout for the requested
module and version, 64-bit ELF shared objects, PE DLLs or thin Mach-O
bundles/dylibs, target architecture and the three required OBS exports
(obs_module_load, obs_module_set_pointer, obs_module_ver). It reports
linked libraries without executing code. The module inspection limit is
128 MiB; universal Mach-O is not supported yet. Static inspection does not
establish runtime loadability, ABI compatibility, dependency resolution,
signature validity or the actual OBS version used to build the binary.
