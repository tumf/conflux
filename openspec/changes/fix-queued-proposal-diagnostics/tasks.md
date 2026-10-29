## Implementation Tasks

- [ ] Reconcile existing strict proposal-reader failures to change-local ProcessingError before blocked-only/capacity exits, preserving candidate exclusions and using the same bounded diagnostic and scheduler warning at late dispatch. Reconcile state independently of event-dedup epochs, including pre-adoption lag, and clear epochs on explicit Retry/removal. Completion: no invalid row stays plain queued, no self-dependency/global Error, no analyzer/Apply for that target. (verification: integration - src/parallel/queue_state.rs and cargo test queued_proposal_diagnostic; verification-id: queued-proposal-diagnostic)
- [ ] Add deterministic invalid-only, mixed queue, repeat/projection reconstruction, repaired Retry, invalid Retry, LF/CRLF and valid-empty/genuine-dependency controls. Completion: old code fails; fixed code has exact dispatch/event counts and safe details with no sentinel values. (verification: integration - cargo test queued_proposal_diagnostic; verification-id: queued-proposal-diagnostic)
- [ ] Add src/ module tests selected by the declared --lib command to prove existing API/client-wait and TUI error/details projections carry the diagnostic and release action-needed waits; add normal/narrow TestBackend checks without changing visual design. Completion: correct error/remedy rendered, no invented Acceptance provenance and unrelated rows unchanged. (verification: integration - cargo test queued_proposal_diagnostic across src/orchestration and src/tui; verification-id: queued-proposal-diagnostic)

## Final Validation

Run `cflx openspec validate fix-queued-proposal-diagnostics --archive-gate`. The declared gate must select nonzero tests. Archive preserves the canonical contract and does not reference a removed active-path runner.
