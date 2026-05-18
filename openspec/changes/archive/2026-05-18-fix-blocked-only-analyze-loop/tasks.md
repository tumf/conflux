## Implementation Tasks

- [x] Add a scheduler-local blocked-only classification for queued candidates in `src/parallel/queue_state.rs`, distinguishing ordinary dispatchable apply candidates from manual `MergeWait`, reducer-owned lane waiters, terminal-error retry-required rows, dependency-blocked rows, and missing candidates. (verification: unit - add focused classifier tests in `src/parallel/tests/executor.rs` that assert each class from reducer/workspace evidence)
- [x] Gate `perform_reanalysis_and_dispatch` in `src/parallel/queue_state.rs` so it skips `analyze_command` when the classifier finds no ordinary dispatchable candidates. (verification: unit - add `src/parallel/tests/executor.rs` test with an analyzer panic/call counter proving merge-wait-only and terminal-error-only queues do not call the analyzer)
- [x] Update finite scheduler drain behavior in `src/parallel/orchestration.rs` so blocked-only queued work exits the running loop without redispatching ordinary apply work. (verification: unit - add `src/parallel/tests/executor.rs` finite scheduler regression test that leaves only `MergeWait`/blocked work and asserts the loop reaches completion without repeated analysis)
- [x] Update persistent scheduler idle behavior in `src/parallel/orchestration.rs` so blocked-only queued work enters event-driven idle wait and wakes only on existing queue/retry notifications. (verification: unit - extend `src/parallel/tests/executor.rs` persistent idle tests to assert no timer-driven worktree reconciliation or analysis occurs while blocked-only state is stable)
- [x] Deduplicate stable analyze-command failure diagnostics in `src/parallel/queue_state.rs` by queued/in-flight/error signature without using durable workflow-control state. (verification: unit - add `src/parallel/tests/executor.rs` test that simulates repeated identical analysis failure and asserts a single operator-visible diagnostic while later changed signatures emit again)
- [x] Preserve manual `MergeWait` retry semantics so queue reconciliation logs `manual_merge_wait` but does not add ordinary queued candidates until explicit `ResolveMerge` promotes the change to scheduler-owned retry work. (verification: integration - extend existing merge-wait reconciliation tests in `src/parallel/tests/executor.rs` to cover blocked-only drain plus subsequent explicit retry)
- [x] Run targeted Rust tests for the affected scheduler/reconciliation behavior. (verification: integration - `cargo test parallel::tests::executor -- --nocapture` or narrower matching test filters if runtime exceeds the default suite budget)

## Future Work

- If operators need a distinct UI state beyond existing logs, consider adding a non-breaking `AllSettledWithBlocked` event in a separate proposal.

## Final Validation

Archive validation is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-blocked-only-analyze-loop --archive-gate`

## Acceptance #1 Failure Follow-up
- [x] Archive commit path was blocked by the repository pre-commit hook. The real hook at `/Users/tumf/work/conflux/.git/hooks/pre-commit` runs `prek hook-impl`, and `.pre-commit-config.yaml:23-28` configures `clippy` as `cargo clippy --locked --all-targets --all-features -- -D warnings`. Manual commit-path equivalent command `agent-exec run -- prek run --all-files` failed with exit code 1. Evidence: `/Users/tumf/.local/share/agent-exec/jobs/6a9c9f9be7f79f1779aeae49b8240ea5/stdout.log:11-22` reported `clippy::type-complexity` at `src/parallel_run_service.rs:46:39` for `analyze_failure_diagnostics_seen: Arc<Mutex<HashSet<(Vec<String>, Vec<String>, String)>>>`. Fixed in-repo by introducing descriptive `AnalyzeFailureDiagnosticSignature` and `AnalyzeFailureDiagnosticStore` type aliases in `src/parallel_run_service.rs:24-25`, and using `AnalyzeFailureDiagnosticStore` for `analyze_failure_diagnostics_seen` in `src/parallel_run_service.rs:49`. (verification: manual - runnable command `cargo clippy --locked --all-targets --all-features -- -D warnings` exited 0 via agent-exec job `1e12484a027efca6e66745452568ce8c`)

## Acceptance #2 Failure Follow-up

Archive validation initially failed because the Acceptance #1 follow-up checkbox lacked the explicit verification note required by the archive gate. The Acceptance #1 follow-up now includes `(verification: manual - ...)` inline evidence, and the previous clippy blocker remains fixed: `src/parallel_run_service.rs:24-25` defines `AnalyzeFailureDiagnosticSignature` and `AnalyzeFailureDiagnosticStore`, `src/parallel_run_service.rs:49` uses the store alias, and `agent-exec run -- cargo clippy --locked --all-targets --all-features -- -D warnings` exited 0 (job `1e12484a027efca6e66745452568ce8c`).

## Acceptance #3 Failure Follow-up

Archive validation was blocked by the OpenSpec archive gate because `openspec/changes/fix-blocked-only-analyze-loop/tasks.md:21` used `verification:` without the required literal parenthesized `(verification: ...)` format. The Acceptance #1 follow-up was rewritten to use an inline parenthesized verification note.

## Acceptance #4 Failure Follow-up

Archive validation was still blocked by the OpenSpec archive gate. Command `cflx openspec validate fix-blocked-only-analyze-loop --archive-gate` failed with repository-fixable `tasks.md` issues: (1) `openspec/changes/fix-blocked-only-analyze-loop/tasks.md:21` verification note was parenthesized, but the archive gate rejected it because it did not cite repository-verifiable evidence such as source paths, tests, or runnable commands. The note now cites `src/parallel_run_service.rs:24-25`, `src/parallel_run_service.rs:49`, and runnable command `cargo clippy --locked --all-targets --all-features -- -D warnings`. (2) self-referential final OpenSpec validation checkboxes were present at previous lines `28` and `31`; final OpenSpec validation evidence is now narrative under the non-checkbox `## Final Validation` / acceptance follow-up sections. Commit-path hooks themselves passed: `agent-exec run -- prek run --all-files` job `ba135daa8b9fac84d45f3b5c98d8fc01` exited 0 with trailing-whitespace, EOF, YAML/TOML, large-file, rustfmt, and clippy all passed. Final `git status --porcelain=v1` was clean.
