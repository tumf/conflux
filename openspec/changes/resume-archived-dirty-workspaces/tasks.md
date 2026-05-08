## Implementation Tasks

- [ ] Task 1: Define reducer/runtime semantics for `Archiving (files moved, commit incomplete)` so archived dirty workspaces remain recoverable and scheduler-ownable after archive finalization failure. (verification: unit - state/reducer tests prove archived dirty observation is preserved as recoverable state instead of collapsing immediately into terminal idle/error)
- [ ] Task 2: Update workspace state discovery and scheduler candidate selection so a workspace with archive files present and active change directory absent can be rediscovered and queued for archive-finalization repair on later scheduler cycles. (verification: integration - scheduler tests simulate a persisted archived-dirty worktree after a failed run and assert the next cycle reclaims it as repair work)
- [ ] Task 3: Add a dedicated retry/ownership path for archived dirty workspaces that resumes archive finalization without re-running the full archive command unless archive file-state regression is detected. (verification: integration - parallel archive test proves a second scheduler cycle resumes finalization only, while a regressed archive move still requires the full archive path)
- [ ] Task 4: Distinguish archive command failure, archive finalization retry, archived dirty recoverable hold, and exhausted terminal archive failure in events/logging/display state. (verification: integration - event/log tests assert separate user-visible outputs for archive move retry vs archived dirty repair vs final terminal failure)
- [ ] Task 5: Ensure terminal `Archive failed` is emitted only after archived dirty recovery policy is exhausted, not merely because one run ended with `Archive commit verification failed`. (verification: integration - test a first run ending in archive finalization failure followed by a second scheduler cycle that resumes repair instead of staying terminal)
- [ ] Task 6: Add regression coverage for the observed `fix-dependency-target-handling` shape: archive move done, dirty archive workspace remains, scheduler previously idled, and the new behavior resumes repair work. (verification: integration - executor/orchestration test fixture reproduces archive rename + dirty spec/tasks/report files and asserts scheduler resumes ownership)
- [ ] Task 7: Run targeted Rust verification for orchestration/archive recovery behavior. (verification: integration - run the relevant `parallel::tests::executor`, orchestration-state, and archive recovery test targets covering archived dirty rediscovery)

## Future Work

- Consider whether archived dirty recovery should expose an explicit TUI action in addition to automatic scheduler reclamation.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate resume-archived-dirty-workspaces --archive-gate`
