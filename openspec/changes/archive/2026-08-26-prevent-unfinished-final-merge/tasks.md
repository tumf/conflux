## Implementation Tasks

- [x] Add repository-derived task completion evidence for active and archived sequential-merge items, failing closed when completion cannot be established. (verification: unit - focused tests beside `src/parallel/resolve_state.rs`, rerun with `cargo test --locked parallel::resolve_state`; verification-id: merge-authorization-tests)
- [x] Add one non-agent-actionable merge-not-authorized classification before final-merge guidance, reuse the existing evidence-withheld/manual-action terminal path, and emit no imperative merge command for incomplete changes. (verification: unit - classification and diagnosis assertions beside `src/parallel/resolve_state.rs`, rerun with `cargo test --locked parallel::resolve_state`; verification-id: merge-authorization-tests)
- [x] Preserve a typed resolver safety refusal as an in-process monotonic stop latch so unchanged evidence cannot launch another agent attempt in the same batch; do not parse narrative output. (verification: integration - retry harness in `src/parallel/conflict.rs` tests proves one attempt and no repeated diagnosis, rerun with `cargo test --locked parallel::conflict`; verification-id: merge-authorization-tests)
- [x] Cover incomplete active and archived tasks, conflict-resolved-but-not-authorized, complete-task green merge, and unrelated batch item non-mutation without broadening the workflow state model. (verification: integration - focused cases in `src/parallel/resolve_state.rs` and `src/parallel/conflict.rs`, rerun with `cargo test --locked parallel::resolve_state parallel::conflict`; verification-id: merge-authorization-tests)

## Final Validation

Expected archive gate: `cflx openspec validate prevent-unfinished-final-merge --archive-gate`.

## Notes

- Implementation: `BatchState::MergeNotAuthorized` (`src/parallel/resolve_state.rs`) is the single typed
  non-authorized outcome. It answers `allows_agent_action() == false`, so it leaves through the existing
  `EvidenceWithheld` terminal path in `src/parallel/conflict.rs` with zero further agent attempts, and its
  diagnosis carries no `git merge`, no `--no-ff`, and no final-commit subject.
- Evidence: `read_task_completion` parses the validated branch tip's own committed `tasks.md` — active
  (`openspec/changes/<id>/tasks.md`) or archived (`openspec/changes/archive/<entry>/tasks.md`) — through
  `crate::task_parser::parse_content`. Missing, unreadable, `0/0`, or read-error evidence answers
  `TaskCompletion::Unestablished` and withholds the merge.
- Gate placement: immediately before `FinalMergeMissing`, after pre-sync, conflict, and archive-identity
  checks. Conflict resolution therefore stays available while the merge is withheld, and an unfinished
  change can never reach the state where a final merge is in progress.
- Latch: `MergeAuthorizationLatch` is a process-local `Mutex<BTreeMap>` owned by one
  `resolve_merges_with_retry` call. It only ever gains refusals, authorizes nothing, and disappears with
  the process, so the next run recomputes authorization from the workspace alone
  (`openspec/CONSTITUTION.md` law 1). Narrative output is never parsed.
- evidence: `cargo test --locked parallel::resolve_state` — 16 passed, 0 failed
- evidence: `cargo test --locked parallel::conflict` — 10 passed, 0 failed
- evidence: `cargo test --locked` — 4387 passed, 0 failed
- evidence: `cargo clippy --locked --all-targets` — no warnings; `cargo fmt --all` applied

## Future Work

Record acceptance execution revisions without making them a merge gate. Evaluate freshness gating separately after observing pre-sync false-block behavior.
