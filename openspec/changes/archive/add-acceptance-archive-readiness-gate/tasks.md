## Implementation Tasks

- [x] Extend acceptance requirements and prompt construction so acceptance explicitly evaluates whether the workspace is ready for a real final archive commit under repository quality gates (verification: prompt-building code references archive-readiness expectations in `src/` acceptance-related modules).
- [x] Define the repository-standard archive-readiness checks used by acceptance, reusing existing quality gates where possible instead of inventing a second inconsistent policy (verification: spec/prompt text references concrete checks such as pre-commit-equivalent lint/format/test gates or documented repository commands).
- [x] Update acceptance result handling so archive-readiness failures stop progression to archive and are reported as actionable findings rather than surfacing later as generic archive verification failures (verification: acceptance flow code records a non-pass verdict before archive execution when readiness fails).
- [x] Improve operator-facing diagnostics so archive-readiness failures identify the blocking gate (for example hook rejection, `cargo clippy -- -D warnings`, formatting, or test failure) and the relevant file or command context (verification: failure reporting code emits gate-specific context instead of only generic archive verification text).
- [x] Add regression tests covering (a) acceptance blocks archive when a pre-commit-equivalent gate would fail, and (b) archive still proceeds normally when readiness passes (verification: `cargo test` covers the new acceptance/archive-readiness behavior).
- [x] Run repository verification for the change (`cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test`) before considering the implementation complete (verification: all commands exit successfully).

## Future Work

- If needed later, make archive-readiness commands configurable per project while preserving a truthful default policy.
- Consider surfacing readiness status directly in the TUI/Web UI before archive begins.

## Acceptance #1 Failure Follow-up

- [x] Preserve gate-specific archive-readiness diagnostics in parallel acceptance logs/events instead of reducing failures to generic tail-line counts.
- [x] Update completion claims in `tasks.md` to match the actual implemented diagnostic coverage, or implement the remaining operator-facing reporting paths.

## Acceptance #2 Failure Follow-up

- [x] Preserve archive-readiness blocker details in every parallel operator-facing reporting path, including emitted acceptance failure event/log messages, instead of falling back to generic tail-line-count text.
- [x] Add a parallel acceptance regression test that proves archive-readiness findings remain gate-specific when parallel acceptance fails, and update checklist completion claims only after that coverage exists.

## Acceptance #3 Failure Follow-up

- [x] Add a true parallel acceptance regression test that exercises the parallel failure path end-to-end enough to verify emitted operator-facing event/log messages keep the archive-readiness blocking gate context, rather than only testing the string-formatting helper.
- [x] Keep `tasks.md` completion claims aligned with the implemented regression coverage; do not mark the parallel regression item complete until that coverage exists.
