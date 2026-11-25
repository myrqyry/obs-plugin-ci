# Current checkpoint

Desktop audit identified original OBS-template workflows, pinned SDK
verification workflows, and specialist codec/inference pipelines. Several
local checkouts are alternate release or older development copies; avoid
migrating duplicates. Streaming Media merged baseline 2fbb807 passed all
eight hosted jobs but run 45554656640 uploaded zero artifacts.

First pilot: one reusable workflow with a consistent eight-job matrix,
archive names, checksum handoff and 14-day test artifacts. Red/green
contract tests passed; an actual older verified Linux archive passed the
handoff after name normalization. Streaming Media uses a temporary local
copy and its existing verified native drivers. No hosted run yet.

Next: publish shared workflow when authorized, run the Streaming Media
pilot on GitHub, confirm eight downloadable archives, exact payloads and
SHA-256 files; switch the caller to the shared immutable commit. Then
generalize SDK preparation, native verifier and package policies from
actual Timeline, Framerot, emuLens, HyperShader and other active repos.
Do not remove specialized CI until replacement passes. No releases.

## Published pilot

Owner authorized publication and qualification. Public repository created
at https://github.com/myrqyry/obs-plugin-ci, main db6c0a2. Streaming Media
branch ci/shared-plugin-build 367c131 calls the exact published workflow
commit; duplicate local workflow removed. Hosted run 45558327793 started
all eight jobs. Verify completed jobs and download every artifact, check
names, exact contents, and SHA256SUMS before merging or migrating callers.

Next candidate: ~/loam/framerot-release (clean, tracks myrqyry/framerot).
Its existing Linux CI has 18 native/codec/GPU tests and publishes binary
and source archives plus two checksum manifests. It installs both OBS
Development and Runtime components and builds libobs-opengl; retain this
GPU test environment and source-distribution artifact. Separate Windows
and macOS codec qualification tests do not yet build/install full plugins.
Do not replace them with an unqualified eight-job plugin matrix. Extend
shared contract to accommodate independently qualified platform subsets
and supplemental artifacts without weakening exact payload verification.
HyperShader and Timeline local checkouts have unpublished commits; do
not sweep those into a CI migration. emuLens-release is clean but uses
template packaging with plugin-specific GPU tests and resources.

## Public package naming

Owner requests streaming-media-source instead of obs-streaming-media for
public names. Run 45558327793 completed all eight jobs and returned eight
artifacts via the Actions API. Added optional package-name input, preserving
module name and verified binary contents; verified archive is renamed and
checksum manifest rewritten after the original integrity gate. Contract
regressions 4/4 passed. Next caller revision pins this change; old run
artifacts retain their existing names. Hosted naming qualification pending.

## Qualified artifact handoff (2026-11-25)

Streaming Media branch ci/shared-plugin-build, commit 875f6b1, hosted run
45566510651 completed all eight jobs successfully. Downloaded all eight
artifacts to its ignored build/ci-artifact-audit.EdpbBG directory. Every
outer artifact has the streaming-media-source public name, exactly one
matching inner archive and SHA256SUMS. All eight SHA-256 digests matched.
All ZIPs passed integrity tests. Inspected archive contents: three files
per Linux/Windows archive (module and two license files), five per macOS
bundle (module, two licenses, Info.plist and ad-hoc signature resources).
OBS32 Linux/Windows legacy and OBS33 modern layout confirmed. No actual
cross-platform runtime install or playback claimed. Public naming and
shared artifact contract are qualified; no merge performed.

Next caller remains Framerot, from clean framerot-release. emuLens-release
and obs-retro-effects are also clean. HyperShader has 12 unpublished
commits, Timeline one, Aftermarket checkout 19 and a modified source
file; isolate future CI work from those changes. No other caller adopted
the shared workflow yet.

## Framerot migration slice

Extended shared build.yml with a validated full/linux-obs32 matrix selector,
caller-owned verifier driver, and separately verified optional source archive.
Preserves the native default driver's eight target/version combinations.
Six contract tests pass (including actual plan-step execution, invalid inputs,
checksum handoff and missing/corrupt source artifacts). Framerot's existing
pinned SDK, 18 tests, install payload and source checks remain caller-owned.
Hosted qualification of this new workflow revision is pending.

Framerot caller pushed at 1dbe750 on ci/shared-plugin-build, pins shared
afeed7458c9fa5da341cc464042fa0f878ecf33c. Existing SDK/GPU/codec build
script unchanged. All five Framerot infrastructure tests passed (one
archive adapter, three CTest receipt, one README link test); six shared
contract tests passed. Downloaded previous successful run 45418891803
to /tmp/framerot-shared-ci-audit.bFTm1h; actual old archive bytes passed
both new shared gates, and all seven binary payload file digests matched.
The new hosted run must complete the real build and all 18 native tests;
verify both uploaded archives and receipts before merging. Codec-only
Windows/macOS qualification workflow unchanged. No other plugin migrated.

## Framerot hosted qualification (2026-11-25)

Run 45577168933 at caller 1dbe750, shared afeed74, succeeded on
Linux x86_64 / OBS 32.2.0 with all 18 native tests passing. Downloaded
both artifacts to /tmp/framerot-hosted-audit.odZZKx: exact outer/inner
file sets, matching SHA256SUMS, all seven installed-file digests match
PAYLOAD-SHA256SUMS, and the source tar contains all 63 tracked file
paths and no extras. No Windows/macOS full plugin qualification or
runtime OBS installation claimed. No merge performed.

## Shared contract hardening

External architecture audit identified late input validation and weak payload
manifest path validation. Reproduced 16 failing regression subcases, moved
existing identifier/test-floor validation to plan without altering accepted
inputs, and reject unsafe or duplicate payload paths. Seven contract tests
pass; the new source gate accepts the actual hosted Framerot artifact.
New revision is local-only and requires hosted qualification before callers
update pins. Next: common archive structural validation against real pilot
archives, cross-platform case-only rename handling, self-testing CI, then
shared SDK provisioning and caching. Preserve caller-specific verifiers.

## Binary archive gate and case-only renames

Added independent binary archive structure and checksum validation before
public rename/upload. Reject empty payloads, malformed/truncated containers,
unsafe paths, links/special members, duplicate and case-colliding names,
file/directory conflicts and selected forbidden payloads. Keep exact layout,
license and resource policy caller-owned. Case-only rename now compares
literal names and distinguishes a filesystem alias from a real destination;
uses a temporary intermediate and preserves original bytes on rollback.
Regression tests reproduced checksum-valid nonarchives and simulated
case-insensitive alias collision before fixes. Ten contract tests pass.
Replayed the new gate against all eight downloaded Streaming Media archives
from run 45566510651 and the Framerot binary from run 45577168933:
all nine passed, SHA-256 bytes unchanged. Native Windows/macOS execution
of the rename remains unqualified; source archive structural inspection,
self-testing workflow and reusable SDK provisioning remain next.
No callers changed or pushed.

## Source verification and self-testing CI

Added source tar/gzip readability and single-root validation, safe paths,
regular files/directories only, root LICENSE/COPYING and common compiled
binary suffix exclusions. Full tracked-file/license/payload policy stays
caller-owned. Fixed prior summary newline escape bug, reproduced with an
explicit GITHUB_STEP_SUMMARY regression test. Replayed nine hosted binary
archives with real summary paths and verified Framerot hosted source against
the final source policy. Twelve local contract tests passed. Added read-only
ci.yml running tests on all four runner families, pinned checkout and
setup-python v6 SHA, Python 3.11, pinned PyYAML test dependency. Actual
native case-only rename test is cross-platform; alias simulation only runs
on case-sensitive filesystems. Hosted self-CI has not run; no publishing
or downstream pin changes. Next: matrix/source host selection, synthetic
workflow-call fixture, and SDK provisioning extraction based on real callers.
