## Implementation Tasks

- [ ] Add `command_max_runtime_secs` to configuration types, merge precedence, defaults, validation, generated configuration examples, and `CommandQueueConfig`; complete when `0` disables the deadline and a continuously-outputting fixture is still bounded by the configured absolute duration (verification: integration - `cargo test --locked --test process_cleanup_test`; verification-id: command-runtime-tests)
- [ ] Extend the common streaming command runner with a typed absolute-runtime timeout that closes retry admission, terminates the owned process group through SIGTERM/SIGKILL cleanup, proves quiescence, and returns a non-retryable outcome distinct from inactivity timeout and crash (verification: integration - `cargo test --locked --test process_cleanup_test`; verification-id: command-runtime-tests)
- [ ] Refactor Apply cancellation and runtime-limit handling to snapshot dirty managed-worktree progress only after process-group quiescence, retain workspace contents on snapshot failure, and return a typed terminal result that cannot trigger same-run redispatch (verification: unit - `cargo test --locked execution::apply::tests`; verification-id: apply-interruption-tests)
- [ ] Add restart-focused Apply tests proving staged, unstaged, and untracked progress survives interruption in a WIP commit and that the next process derives Apply continuation from workspace and Git evidence alone (verification: unit - `cargo test --locked execution::apply::tests`; verification-id: apply-interruption-tests)
- [ ] Route TUI SIGINT and SIGTERM through `TuiRunSupervisor` cancellation and the run-command-scope shutdown barrier before process exit, including cleanup failure diagnostics and non-zero exit when quiescence cannot be proven (verification: unit - `cargo test --locked tui::run_supervisor::tests`; verification-id: tui-shutdown-tests)
- [ ] Add TUI shutdown tests proving external signals close command admission, suppress retries, drain registered executions, and leave no owned process identity after the bounded cleanup path completes (verification: unit - `cargo test --locked tui::run_supervisor::tests`; verification-id: tui-shutdown-tests)
- [ ] Update `skills/cflx-apply/SKILL.md` and its reference guidance to require single-run verification by default, prohibit no-change stability loops, cap identical verification commands at three evidence-bearing executions, and emit structured `verification_timeout` or `verification_unstable` blocker facts instead of waiting indefinitely (verification: unit - `cargo test --locked --test install_skills_test`; verification-id: skill-contract-tests)
- [ ] Update `skills/cflx-proposal/SKILL.md` so heavy, Docker, database, credentialed, deployed, and long-running repository-wide gates are assigned to CI, Acceptance, or operational-observation ownership rather than Apply-blocking checkbox tasks unless a bounded repository-local path is declared (verification: unit - `cargo test --locked --test install_skills_test`; verification-id: skill-contract-tests)
- [ ] Extend embedded-skill contract tests to reject guidance regressions that permit unchanged verification loops, omit bounded blocker handoff, or assign non-local heavy gates as change-blocking Apply work (verification: unit - `cargo test --locked --test install_skills_test`; verification-id: skill-contract-tests)

## Future Work

- Operational monitoring may later surface command runtime-limit frequency and duration distributions, but metrics remain non-authoritative and are not required for this change.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate prevent-runaway-apply-execution --archive-gate`.
