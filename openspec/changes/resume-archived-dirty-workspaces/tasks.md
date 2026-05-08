## Implementation Tasks

- [x] Task 1: Define reducer/runtime semantics for `Archiving (files moved, commit incomplete)` so archived dirty workspaces remain recoverable and scheduler-ownable after archive finalization failure. (verification: unit - state/reducer tests prove archived dirty observation is preserved as recoverable state instead of collapsing immediately into terminal idle/error)
- [x] Task 2: Update workspace state discovery and scheduler candidate selection so a workspace with archive files present and active change directory absent can be rediscovered and queued for archive-finalization repair on later scheduler cycles. (verification: integration - scheduler tests simulate a persisted archived-dirty worktree after a failed run and assert the next cycle reclaims it as repair work)
- [x] Task 3: Add a dedicated retry/ownership path for archived dirty workspaces that resumes archive finalization without re-running the full archive command unless archive file-state regression is detected. (verification: integration - parallel archive test proves a second scheduler cycle resumes finalization only, while a regressed archive move still requires the full archive path)
- [x] Task 4: Distinguish archive command failure, archive finalization retry, archived dirty recoverable hold, and exhausted terminal archive failure in events/logging/display state. (verification: integration - event/log tests assert separate user-visible outputs for archive move retry vs archived dirty repair vs final terminal failure)
- [x] Task 5: Ensure terminal `Archive failed` is emitted only after archived dirty recovery policy is exhausted, not merely because one run ended with `Archive commit verification failed`. (verification: integration - test a first run ending in archive finalization failure followed by a second scheduler cycle that resumes repair instead of staying terminal)
- [x] Task 6: Add regression coverage for the observed `fix-dependency-target-handling` shape: archive move done, dirty archive workspace remains, scheduler previously idled, and the new behavior resumes repair work. (verification: integration - executor/orchestration test fixture reproduces archive rename + dirty spec/tasks/report files and asserts scheduler resumes ownership)
- [x] Task 7: Run targeted Rust verification for orchestration/archive recovery behavior. (verification: integration - ran `cargo test orchestration::state::tests::archive_resumed_clears_recoverable_archive_error_and_restores_archiving_activity`, `cargo test parallel::dispatch::tests::archived_dirty_repair_candidate_reads_archived_tasks_without_active_change_dir`, and `cargo test parallel::tests::executor::test_archived_dirty_finalization_resume_does_not_rerun_archive_command`)

## Future Work

- Consider whether archived dirty recovery should expose an explicit TUI action in addition to automatic scheduler reclamation.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate resume-archived-dirty-workspaces --archive-gate`
