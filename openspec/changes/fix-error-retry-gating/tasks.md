## Implementation Tasks

- [x] Add reducer-owned explicit retry transition for terminal-error changes in `src/orchestration/state.rs` (completion: a public/internal reducer method or command path clears `TerminalState::Error`, restores `QueueIntent::Queued`, clears stale wait metadata, and leaves rejected/merged/archived terminal states immutable; verification: unit - reducer tests show error changes are excluded before retry, retry clears only recoverable error, and rejected/merged changes cannot be retried into apply).

- [x] Gate parallel ordinary apply dispatch against reducer terminal-error state in `src/parallel/queue_state.rs` (completion: selection/reanalysis cannot include a change whose reducer display/runtime state is terminal `Error` unless explicit retry intent has cleared that terminal state; verification: integration - parallel queue tests prove `ProcessingError` followed by reanalysis does not dispatch the same change again).

- [x] Gate workspace resume and repair-candidate dispatch against terminal-error state in `src/parallel/dispatch.rs` and related resume scan paths (completion: existing errored worktrees are preserved for operator inspection but are not converted into ordinary apply jobs solely because they still exist or look repairable; verification: integration - workspace resume tests cover an errored workspace remaining `error`/not-dispatched until explicit retry).

- [x] Wire TUI retry-mark/F5 behavior to the explicit reducer retry transition in `src/tui/state/selection_logic.rs` and command handling paths (completion: retrying marked error rows clears reducer error terminal state and requeues exactly the marked changes; unmarked error rows remain gated; verification: unit - TUI selection tests prove retry-mark requeues errored rows through reducer state and leaves unmarked error rows stopped).

- [x] Preserve delayed success supersession semantics for recoverable errors (completion: `ChangeArchived`, `MergeCompleted`, and `ResolveCompleted` from an already-running same-change execution can still supersede `TerminalState::Error` without requiring explicit retry, but do not create a fresh apply dispatch; verification: unit - reducer tests cover late archive/merge/resolve success after error and assert no queue intent is reintroduced for ordinary apply).

- [x] Keep dependency-blocked behavior consistent with errored dependencies (completion: dependents of an errored change remain blocked/skipped until the dependency is explicitly retried and reaches repository-visible success, after which normal dependency reanalysis may unblock them; verification: integration - parallel dependency tests cover errored dependency blocks dependent dispatch and explicit retry/success allows reanalysis).

- [x] Run formatting and focused regression tests (verification: manual - run `cargo fmt --check` plus focused `cargo test` invocations for reducer, parallel queue/dispatch/resume, and TUI retry; completion: command outputs pass locally or document any intentionally skipped heavy tests).

## Future Work

- Broader UX improvements for surfacing retry guidance in the web dashboard can be proposed separately if needed.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-error-retry-gating --archive-gate`

## Acceptance #1 Failure Follow-up
- [x] src/parallel/queue_state.rs:387-465 only checks the candidate change itself for reducer terminal-error gating (lines 374-383) and then classifies dependencies via queued/in-flight/archived/rejected/missing plus repository-visible resolution. It never checks whether a dependency ID is in reducer TerminalState::Error. Therefore a dependent can still dispatch when its dependency is errored but the dependency target still exists as a normal queued/known change and is not in flight. This violates the acceptance criterion and spec delta that dependents of an errored dependency remain blocked until the dependency is explicitly retried and reaches repository-visible success. The focused search also found no regression test for this exact scenario (the attempted `cargo test dependency_on_terminal_error` matched 0 tests), so add dispatch dependency-blocking logic against shared reducer terminal-error state and a test where beta depends on alpha, alpha is TerminalState::Error, beta is not selected until alpha is retried and repository-visible success is established. (verification: integration - `cargo test dependency_on_terminal_error --lib` passed locally, 1 test, 0 failed).

## Acceptance #2 Failure Follow-up
- [x] openspec/changes/fix-error-retry-gating/tasks.md:27 blocks the repository’s declared final archive gate. Running `agent-exec run -- cflx openspec validate fix-error-retry-gating --archive-gate` exited 1 with `fix-error-retry-gating: tasks.md:27: Behavior-bearing task missing '(verification: ...)' note`. The added Acceptance #1 follow-up checklist item used `Verification:` instead of the archive-gate-required `(verification: ...)` note format, so the real archive validation/commit path was not ready. Rewrote the Acceptance #1 follow-up checklist entry to include an explicit `(verification: integration - cargo test dependency_on_terminal_error --lib passed locally, 1 test, 0 failed)` note that satisfies the archive gate. (verification: manual - this task is checklist/schema normalization for the archive gate; `cflx openspec validate fix-error-retry-gating --archive-gate` must pass after this rewrite).

## Acceptance #3 Failure Follow-up

Acceptance #3 found a checklist-format issue in the Acceptance #2 follow-up entry: it was written as a behavior-bearing checkbox whose verification note pointed back to the final archive gate, so the archive gate correctly treated it as self-referential final validation. The Acceptance #2 follow-up evidence has been preserved as historical context, and this Acceptance #3 note is intentionally non-checkbox narrative because final OpenSpec validation is already owned by the non-checkbox `## Final Validation` section above.
