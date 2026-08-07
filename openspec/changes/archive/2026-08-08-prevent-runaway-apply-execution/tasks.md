## Implementation Tasks

- [x] Add `command_max_runtime_secs` to configuration types, merge precedence, defaults, validation, generated configuration examples, and `CommandQueueConfig`; complete when `0` disables the deadline and a continuously-outputting fixture is still bounded by the configured absolute duration. Validation follows the same model as every sibling command knob (`command_inactivity_timeout_secs`): any `u64` is accepted and `0` is the explicit disable, so the enforceable rule is the `0`-disable semantics rather than a new rejection path (verification: integration - `cargo test --locked --features heavy-tests --test process_cleanup_test absolute_runtime_limit`; verification-id: command-runtime-tests)
- [x] Extend the common streaming command runner with a typed absolute-runtime timeout that closes retry admission, terminates the owned process group through SIGTERM/SIGKILL cleanup, proves quiescence, and returns a non-retryable outcome distinct from inactivity timeout and crash (verification: integration - `cargo test --locked --features heavy-tests --test process_cleanup_test absolute_runtime_limit`; verification-id: command-runtime-tests)
- [x] Refactor Apply cancellation and runtime-limit handling to snapshot dirty managed-worktree progress only after process-group quiescence, retain workspace contents on snapshot failure, and return a typed terminal result that cannot trigger same-run redispatch (verification: unit - `cargo test --locked execution::apply::tests::interrupted_apply`; verification-id: apply-interruption-tests)
- [x] Add restart-focused Apply tests proving staged, unstaged, and untracked progress survives interruption in a WIP commit and that the next process derives Apply continuation from workspace and Git evidence alone. Per design.md these are Git-backed and therefore integration-shaped evidence, kept in `execution::apply::tests::interrupted_apply_restart` alongside the unit-scoped decision tests in `interrupted_apply` (verification: unit + integration - `cargo test --locked execution::apply::tests::interrupted_apply`; verification-id: apply-interruption-tests)
- [x] Route TUI SIGINT and SIGTERM through `TuiRunSupervisor` cancellation and the run-command-scope shutdown barrier before process exit, including cleanup failure diagnostics and non-zero exit when quiescence cannot be proven (verification: unit - `cargo test --locked tui::run_supervisor::tests`; verification-id: tui-shutdown-tests)
- [x] Add TUI shutdown tests proving external signals close command admission, suppress retries, drain registered executions, and leave no owned process identity after the bounded cleanup path completes (verification: unit - `cargo test --locked tui::run_supervisor::tests`; verification-id: tui-shutdown-tests)
- [x] Update `skills/cflx-apply/SKILL.md` and its reference guidance to require single-run verification by default, prohibit no-change stability loops, cap identical verification commands at three evidence-bearing executions, and emit structured `verification_timeout` or `verification_unstable` blocker facts instead of waiting indefinitely (verification: unit - `cargo test --locked --test install_skills_test`; verification-id: skill-contract-tests)
- [x] Update `skills/cflx-proposal/SKILL.md` so heavy, Docker, database, credentialed, deployed, and long-running repository-wide gates are assigned to CI, Acceptance, or operational-observation ownership rather than Apply-blocking checkbox tasks unless a bounded repository-local path is declared (verification: unit - `cargo test --locked --test install_skills_test`; verification-id: skill-contract-tests)
- [x] Extend embedded-skill contract tests to reject guidance regressions that permit unchanged verification loops, omit bounded blocker handoff, or assign non-local heavy gates as change-blocking Apply work (verification: unit - `cargo test --locked --test install_skills_test`; verification-id: skill-contract-tests)

## Future Work

- Operational monitoring may later surface command runtime-limit frequency and duration distributions, but metrics remain non-authoritative and are not required for this change.

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate prevent-runaway-apply-execution --archive-gate`.

Single-run verification evidence for this apply:

- `cargo test --locked` (default tier): pass, exit 0.
- `cargo test --locked --features heavy-tests --test process_cleanup_test absolute_runtime_limit`: 3 passed, 0 failed.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cflx openspec validate prevent-runaway-apply-execution --strict` and `--archive-gate`: validation passed.
