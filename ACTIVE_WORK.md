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

## Rust CI engine first slice

Owner clarified Rust should be considered for implementing the infrastructure,
not merely for building Rust plugins. Added standalone obs-ci crate with
plan (current matrix and validation contract) and audit (read-only binary
archive validation and JSON receipt). Cargo lockfile retained. Red/green
CLI tests, persistent Python/Rust parity suite, fixed ZIP reader duplicate
name deduplication and Unicode full case folding before checkpoint.
Three Rust integration tests and two parity tests (14 planner cases, 24
archive cases) passed, along with all 12 existing workflow tests and
Clippy. All nine actual hosted binary archives pass final Rust audit;
six valid planner combinations match Python JSON. Self-CI now includes
pinned Rust 1.93.1 checks across the four runner families. Python workflow
remains authoritative. Rust source-package and rename support, composable
selectors and deterministic source host, qualification fixture, and pinned
tool distribution remain open. No publishing or caller migration.

## Rust source audit and matrix filters

Extended obs-ci plan with validated comma-separated targets and pinned OBS
versions. Existing row ordering/fields retained; additive emit_source selects
exactly one Linux job in catalog order, including OBS33-only plans. Reject
unsupported/duplicate selectors, empty intersections and source without Linux.
No claim that arbitrary selections are qualified for a specific plugin.

Added read-only audit-source with source structure and manifest validation
plus optional exact binary payload hash cross-check. Shared archive parser
now hashes/inspects using the same open file handle. Actual Git-generated
Framerot source exposed global PAX comment header; added regression coverage
and limited acceptance to a single leading comment-only header, rejecting
global path overrides. Real hosted source passes with 63 regular files,
11 directories, seven installed hashes verified against the actual binary.
Five Rust tests, three Python/Rust compatibility tests (including 38 prior
cases plus two PAX cases), 12 existing Python tests, formatting, Clippy and
release build passed. Production workflow still uses Python; new selectors
and source auditor are local tooling until hosted qualification. Next:
safe package rename and synthetic workflow-call fixture, followed by
revision-pinned executable distribution and SDK provisioning.

## Rust verified handoff and native binary inspection

Added handoff with pre-audit, portable filename validation, same-extension
requirement, exact source name, destination collision protection including
case-only hard links, staged recovery and atomic manifest replacement.
Tested normal/case-only rename, tampered input, collisions, and simulated
manifest write failure with byte-preserving rollback. Requires isolated
same-filesystem staging and hard-link support; not crash-atomic.

Added inspect-module using goblin, exact OBS32/33 module path, 64-bit
ELF/PE/thin Mach-O architecture and library type checks, three required
OBS exports and linked-library receipts. No code executed; static checks
do not qualify ABI, dependency resolution, code signing or runtime load.
All nine downloaded hosted packages passed native inspection on Linux,
and temporary renamed copies passed checksum and module verification
with unchanged archive SHA-256. Compiled real cdylib fixture exercises
positive and missing-export cases; native Windows/macOS CI still pending.
Eleven Rust tests, three parity tests, 12 workflow tests, formatting,
Clippy and release build passed. No production workflow changes or push.
Next: synthetic workflow-call fixture, revision-pinned executable
distribution, then SDK provisioning/caching.

## Rust workflow qualification fixture

Added manually dispatched qualify.yml calling local rust-fixture.yml at
the same revision. Rust planner selects all eight qualified layouts by
default or validated target/version subsets; source host chosen exactly
once when Linux is selected. Each job builds the locked engine at the
run's SHA, compiles a synthetic Rust cdylib, packages it, runs handoff and
native inspection, uploads exact binary/source archives and receipts.
Collector downloads the full fixture set, checks exact directories/files,
recalculates native and source receipts, rejects mismatches and writes
qualification.json. No actual OBS SDK linkage, runtime load or production
caller change; explicit synthetic-fixture-only report scope.

Red/green local fixture tests include both native OBS layouts, tampered
receipts, unexpected files, changed binary bytes, full/filtered source host
selection. Actual reusable workflow plan/build/collect run blocks replayed
locally on Linux with a simulated artifact download (directory copy);
both layouts and exact source/binary hashes pass. Full suite: 11 Rust,
12 workflow, three parity, two qualification tests (28 methods), fmt,
Clippy, release build and diff check passed. actionlint v1.7.11 accepts
both new workflows; existing ci.yml only flags macos-26-intel as unknown,
verified real successful GitHub job 139042519813 uses that exact label,
so narrowly excluded this known false positive for the combined lint.
No hosted fixture run yet; no push. Next: hosted qualification when
authorized, revision-pinned executable distribution and common SDK
provisioning based on actual callers.

## Same-run revision-pinned engine transport

Qualification now compiles the Linux engine once in the plan job and
one engine per selected non-Linux architecture, not per OBS version.
Producer packages exact binary + ENGINE.json; artifact name includes
run checkout SHA and target. Consumer checks checkout HEAD, host platform,
Cargo.lock digest, pinned Rust version, file size and SHA-256 before
restoring executable permissions. No release or cross-run artifact trust.
Linux collector reuses the plan engine without installing Rust.
The full matrix compiles four engines instead of ten; Linux-only one
instead of four. Additional engine-build jobs and artifact transfers
mean elapsed time improvement is not yet measured. Ubuntu24 engine on
Ubuntu26 and all non-Linux hosted paths remain to be qualified.

Two engine-bundle test methods exercise deduplication, wrong target/revision,
mutated receipt fields, extra files, changed bytes and no overwrite.
All 30 test methods, fmt, Clippy, release build and actionlint passed
(same documented macos-26-intel lint exception). Actual Linux workflow
plan, bundle, simulated download with stripped mode, two fixture builds
and collector shell blocks passed using one release executable. Fresh
dispatch required for clean retry rather than replacing immutable uploads.
Existing production build.yml unchanged; no push. Next: hosted fixture
qualification, shared SDK provisioning/caching based on caller evidence.

## Shared SDK recipe catalog and source cache

Compared actual Streaming Media, UI Augment, Framerot and HyperShader
Linux preparation scripts. Preserve streaming-minimal (32.2.0/33.0.3,
scripting/browser OFF) vs legacy-core (32.2.0/33.0.1, no overrides).
Added Rust sdk-plan with source pins, configure options, build targets,
install components and canonical recipe SHA-256; sdk-verify-source and
sdk-fetch implement verified, content-addressed source archive reuse.
No-clobber temp publication, fail closed on corrupt existing cache, reject
symlinked content directory and nonregular source file. Source identity
does not imply compiled SDK compatibility.

Real upstream sources 32.2.0 (16185428 bytes), 33.0.3 (16258068 bytes)
and 33.0.1 (16255057 bytes) all matched their caller-recorded SHA-256.
32.2.0 cache reused across both recipes; 47 MiB total at
/tmp/obs-ci-sdk-audit.SEI4D0 (temporary verification cache). 35 test methods
on Linux passed (16 Rust, 12 workflow, three parity, two qualification,
two engine bundle), fmt, Clippy, release build and diff check passed.
Desktop root filesystem reported only 5.4 GiB available, so no full
SDK compilation was started. Next: shared CMake execution and host/ABI
fingerprint with installed SDK receipt verification, then hosted pilot.
Production workflow and downstream callers unchanged; no push.

## Shared SDK executor and verified local reuse

Added sdk-fingerprint, sdk-prepare and sdk-verify-installed in Rust.
Conservative Linux x86_64/Debian-compatible fingerprint binds exact
workspace/prefix, recipe, OS, dpkg status and ten tool binary digests.
CMake/Ninja execution keeps original build targets and component install
order, uses system GCC/G++ and isolated environment, records logs.
New workspace only, pinned source verified before extraction, 8 GiB
preflight; no overwrite or repair. Receipt verifies full install tree
including file modes/content and confined relative links, required
configs and version marker. Matching completed workspace reused without
fetch/build; changed fingerprint/tree or missing receipt fails closed.

Actual CMake C/C++ shared-library fixture built/installed with the shared
executor; reuse, modified library, extra file, escaping symlink and
wrong recipe verified. Real host fingerprint repeatable; real low-space
guard rejected new build before source fetch or work creation (5.0 GiB
available). 37 test methods (18 Rust, 12 workflow, three parity, two
qualification, two bundle), Clippy, fmt and release build passed.
No complete OBS SDK build executed; no claim of ABI compatibility or
cross-host cache reuse. Unmanaged dependencies remain outside the
fingerprint. Next hosted qualification and actual OBS SDK pilot, then
evaluate portable SDK/cache receipts and declarative package policies.
Production workflows untouched; no push.

## Shared package writer and Linux-only matrix

Added Rust pack and payload-manifest commands: caller prepares a quiescent
stage; engine writes ZIP or tar.gz, preserves Unix file modes, hashes and
re-reads exact bytes via existing auditor, refuses links, unsafe names,
case collisions and existing output directories. No source policy or ABI
claim implicit. Added linux profile (both OBS32.2.0 and 33.0.3) with Rust
planner and production workflow parity. Copied the exact hosted-qualified
EmuLens Linux SDK script for pinned reuse; experimental SDK executor remains
separate. 40 local test methods passed, fmt, Clippy and release build passed.
No existing caller pin changes. New branch is a candidate for HyperDSP's
Linux-only pilot; hosted qualification not established.

## 2026-11-27: shared checkout preflight, executable transport and Linux SDK reuse

Branch ci/shared-preflight (based on hosted-contract-qualified e9eb704).
Preflight opt-ins checkout-submodules and required-paths validate before
expensive native jobs. Real Git submodule fixture rejected uninitialized
checkout, then passed after initialization. Shared Linux caller accepts a
separate valid public archive name without altering module identity.
Optional engine-revision compiles one locked engine per selected target,
restores only verified same-run executable bundles, and exports
OBS_CI_ENGINE_ROOT for adapters that explicitly opt in. Actual Linux restore
step replayed with real compiled executable, including rejection of tampering.
Existing pins/default caller engine bootstraps unchanged.

Actual Rust SDK executor compiled OBS 32.2.0 legacy-core in 8.832s; verified
exact same receipt on reuse in 0.042s, no source cache access. Optional
linux-sdk-cache requires engine-revision; caller dependency hook precedes
fingerprint, exact cache key includes engine SHA, environment, recipe and
workspace. Cache only installed SDK and receipt, no fallback key. Partial
restored cache fails before plugin compile. Real V4L2 Log Quiet caller built
and passed native CTest and staged-module checks both cold and simulated
restore; archives were byte-identical SHA-256
e502b6b9355c3038381d96c6dbfac765745ceca18cb53b868dcc2365026ba685.
Restored SDK + receipt 2.7 MiB; no source fetch or SDK compilation. Local Linux
OBS32 only, not actual GitHub cache restoration or cross-repo cache sharing.
No SDK compatibility claim for Windows/macOS or OBS33 cache path.

52 local test methods passed (20 Rust, 20 workflow, 4 native caller,
3 parity, 2 synthetic qualification, 3 engine transport); fmt, Clippy,
release build, actionlint and diff check passed. Next: hosted contract
qualification on candidate branch, then a small real caller pilot using
both opt-ins before changing existing production pins. Cross-repository
SDK distribution, targeted production matrix filters, cancellation,
ecosystem dashboard and native runtime qualification remain separate work.

## Hosted cache rejection recovery — Aftermarket run 46110039221
OBS33 cache key matched and GitHub restored the SDK, but its exact receipt
rejected the installed tree; old diagnostics cannot distinguish changed
modes/content from fingerprint mismatch. OBS32 cache miss passed and saved.
Split restore and save, verify or quarantine/rebuild at the exact prefix
before caller compilation, save only a freshly verified miss, retain strict
verification in the actual caller phase. Report the first changed SDK entry
and mode/content distinction. No cache data deletion or relaxed verification.
All 54 local shared checks passed, strict Rust/clippy and actionlint passed.
Real pinned OBS32 SDK rebuilt after deliberately invalid receipt, preserved
rejected bytes, then exact tar transport/restoration passed. OBS33 local
attempt rejected cache and started pinned rebuild but stopped because
desktop FFmpeg 6.1 is below required 8.0; no system changes. Hosted new
namespace and OBS33 rebuild remain to qualify.

## Local OBS33 FFmpeg 8 development path
Linux Mint 22.3/Noble has FFmpeg 6.1.1; existing OBS32 links its ABI.
Installed official FFmpeg 8.1.3 shared build in a separate versioned
user prefix, signature verified against the official fingerprint and
source SHA-256 7138d28c96d9d3e3af4ee3d8cad72741f8ffb40da90c1112235dea3ecd3178a3.
Native H.264/AAC encode/probe/decode passed; ffmpeg8/ffprobe8 expose
the new executables without changing the default ffmpeg. Existing
OBS32 linkage remains FFmpeg 6. Local-only shell SDK prefix opt-in
rejects hosted, OBS32 and invalid prefixes before download; seven
native caller contract tests passed. Real pinned OBS33.0.3 SDK
built successfully with all five header/library paths resolving to
FFmpeg 8.1.3, verified dynamic linkage, copied relocatable installed
SDK to a persistent user-local versioned prefix. No production OBS
installation, hosted cache changes, global linker configuration or
system package upgrades. This local path uses shell SDK preparation,
not the Rust fingerprinted hosted cache.

Aftermarket checkpoint 329df2e: independent clean local OBS33.0.3 / Qt6.4.2 / FFmpeg8.1.3 Release build passed all 49 CTests without skips and verified the four-file modern package, restart helper, archive checksum and ELF exports. The full OBS33 frontend was not installed or launched. Evidence: /tmp/aftermarket-local-obs33.gBeMkP/pilot.log.
