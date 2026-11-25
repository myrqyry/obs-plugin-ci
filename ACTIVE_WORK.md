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
