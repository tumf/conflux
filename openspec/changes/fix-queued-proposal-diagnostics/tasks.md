## Implementation Tasks

- [x] Reconcile existing strict proposal-reader failures to change-local ProcessingError before blocked-only/capacity exits, preserving candidate exclusions and using the same bounded diagnostic and scheduler warning at late dispatch. Reconcile state independently of event-dedup epochs, including pre-adoption lag, and clear epochs on explicit Retry/removal. Completion: no invalid row stays plain queued, no self-dependency/global Error, no analyzer/Apply for that target. (verification: integration - src/parallel/queue_state.rs and cargo test queued_proposal_diagnostic; verification-id: queued-proposal-diagnostic)
- [x] Add deterministic invalid-only, mixed queue, repeat/projection reconstruction, repaired Retry, invalid Retry, LF/CRLF and valid-empty/genuine-dependency controls. Completion: old code fails; fixed code has exact dispatch/event counts and safe details with no sentinel values. (verification: integration - cargo test queued_proposal_diagnostic; verification-id: queued-proposal-diagnostic)
- [x] Add src/ module tests selected by the declared --lib command to prove existing API/client-wait and TUI error/details projections carry the diagnostic and release action-needed waits; add normal/narrow TestBackend checks without changing visual design. Completion: correct error/remedy rendered, no invented Acceptance provenance and unrelated rows unchanged. (verification: integration - cargo test queued_proposal_diagnostic across src/orchestration and src/tui; verification-id: queued-proposal-diagnostic)

## Final Validation

Run `cflx openspec validate fix-queued-proposal-diagnostics --archive-gate`. The declared gate must select nonzero tests. Archive preserves the canonical contract and does not reference a removed active-path runner.

## Notes

- evidence: `cargo test --locked --offline --lib --features web-monitoring queued_proposal_diagnostic` selects 8 tests (6 scheduler in `src/parallel/tests/queued_proposal_diagnostic.rs`, 1 client-wait in `src/client/wait.rs`, 1 TUI TestBackend in `src/tui/render.rs`), all pass.
- evidence: suppressing the new `ProcessingError` emission makes 5 of the 6 scheduler tests fail (the remaining one is the valid-reader control), so the assertions depend on the fix.
- evidence: `cargo clippy --all-targets --features web-monitoring -D warnings` and `cargo fmt` are clean.
- evidence: full `--lib` run: 4862 passed, 4 failed; two pass in isolation (load-sensitive), and `a_bounded_judge_failure_emits_one_redacted_category` plus `production_status_paths_preserve_conflicted_classification` fail identically on the unmodified base commit 323a110a, unrelated to this change.
- The existing `metadata_read_failure_blocks_dispatch_even_without_analyzer_dependencies` test was updated to the new contract (change-local `ProcessingError`, no global `Error`, no self-dependency).
