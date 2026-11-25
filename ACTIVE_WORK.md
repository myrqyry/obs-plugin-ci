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
