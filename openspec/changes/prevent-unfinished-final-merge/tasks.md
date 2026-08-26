## Implementation Tasks

- [ ] Add repository-derived task completion evidence for active and archived sequential-merge items, failing closed when completion cannot be established. (verification: unit - focused tests beside `src/parallel/resolve_state.rs`, rerun with `cargo test --locked parallel::resolve_state`; verification-id: merge-authorization-tests)
- [ ] Add one non-agent-actionable merge-not-authorized classification before final-merge guidance, reuse the existing evidence-withheld/manual-action terminal path, and emit no imperative merge command for incomplete changes. (verification: unit - classification and diagnosis assertions beside `src/parallel/resolve_state.rs`, rerun with `cargo test --locked parallel::resolve_state`; verification-id: merge-authorization-tests)
- [ ] Preserve a typed resolver safety refusal as an in-process monotonic stop latch so unchanged evidence cannot launch another agent attempt in the same batch; do not parse narrative output. (verification: integration - retry harness in `src/parallel/conflict.rs` tests proves one attempt and no repeated diagnosis, rerun with `cargo test --locked parallel::conflict`; verification-id: merge-authorization-tests)
- [ ] Cover incomplete active and archived tasks, conflict-resolved-but-not-authorized, complete-task green merge, and unrelated batch item non-mutation without broadening the workflow state model. (verification: integration - focused cases in `src/parallel/resolve_state.rs` and `src/parallel/conflict.rs`, rerun with `cargo test --locked parallel::resolve_state parallel::conflict`; verification-id: merge-authorization-tests)

## Final Validation

Expected archive gate: `cflx openspec validate prevent-unfinished-final-merge --archive-gate`.

## Future Work

Record acceptance execution revisions without making them a merge gate. Evaluate freshness gating separately after observing pre-sync false-block behavior.
