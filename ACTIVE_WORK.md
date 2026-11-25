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
