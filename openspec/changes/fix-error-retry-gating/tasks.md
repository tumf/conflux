## Implementation Tasks

- [ ] Add reducer-owned explicit retry transition for terminal-error changes in `src/orchestration/state.rs` (completion: a public/internal reducer method or command path clears `TerminalState::Error`, restores `QueueIntent::Queued`, clears stale wait metadata, and leaves rejected/merged/archived terminal states immutable; verification: unit - reducer tests show error changes are excluded before retry, retry clears only recoverable error, and rejected/merged changes cannot be retried into apply).

- [ ] Gate parallel ordinary apply dispatch against reducer terminal-error state in `src/parallel/queue_state.rs` (completion: selection/reanalysis cannot include a change whose reducer display/runtime state is terminal `Error` unless explicit retry intent has cleared that terminal state; verification: integration - parallel queue tests prove `ProcessingError` followed by reanalysis does not dispatch the same change again).

- [ ] Gate workspace resume and repair-candidate dispatch against terminal-error state in `src/parallel/dispatch.rs` and related resume scan paths (completion: existing errored worktrees are preserved for operator inspection but are not converted into ordinary apply jobs solely because they still exist or look repairable; verification: integration - workspace resume tests cover an errored workspace remaining `error`/not-dispatched until explicit retry).

- [ ] Wire TUI retry-mark/F5 behavior to the explicit reducer retry transition in `src/tui/state/selection_logic.rs` and command handling paths (completion: retrying marked error rows clears reducer error terminal state and requeues exactly the marked changes; unmarked error rows remain gated; verification: unit - TUI selection tests prove retry-mark requeues errored rows through reducer state and leaves unmarked error rows stopped).

- [ ] Preserve delayed success supersession semantics for recoverable errors (completion: `ChangeArchived`, `MergeCompleted`, and `ResolveCompleted` from an already-running same-change execution can still supersede `TerminalState::Error` without requiring explicit retry, but do not create a fresh apply dispatch; verification: unit - reducer tests cover late archive/merge/resolve success after error and assert no queue intent is reintroduced for ordinary apply).

- [ ] Keep dependency-blocked behavior consistent with errored dependencies (completion: dependents of an errored change remain blocked/skipped until the dependency is explicitly retried and reaches repository-visible success, after which normal dependency reanalysis may unblock them; verification: integration - parallel dependency tests cover errored dependency blocks dependent dispatch and explicit retry/success allows reanalysis).

- [ ] Run formatting and focused regression tests (verification: manual - run `cargo fmt --check` plus focused `cargo test` invocations for reducer, parallel queue/dispatch/resume, and TUI retry; completion: command outputs pass locally or document any intentionally skipped heavy tests).

## Future Work

- Broader UX improvements for surfacing retry guidance in the web dashboard can be proposed separately if needed.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-error-retry-gating --archive-gate`
