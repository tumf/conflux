## Implementation Tasks

- [x] Add explicit run target parsing in `src/cli.rs`: positional `changes`, `--all`, and legacy `--change` must parse, while target modes are documented as mutually exclusive. (verification: unit - `cargo test cli::tests::test_run_subcommand --lib` covers positional IDs, `--all`, legacy `--change`, and invalid combinations)
- [x] Normalize run targets in the run command path before constructing the orchestrator, preserving `--all` as all-current-changes and positional/`--change` as explicit ID lists. (verification: unit/integration - tests assert `src/main.rs` or a shared helper passes `None` only for `--all` and `Some(Vec<String>)` only for explicit IDs)
- [x] Change serial run snapshot filtering in `src/orchestrator.rs` to fail atomically on unknown or duplicate explicit IDs instead of warning and continuing with a subset. (verification: integration - add a run-mode test under `tests/` or `src/orchestrator.rs` that creates changes `a` and `c`, requests `a missing c`, and asserts non-zero failure with no partial execution)
- [x] Apply the same normalized target filtering and validation to parallel execution and `--parallel --dry-run`, so dry-run plans and parallel worktrees are limited to the requested target set. (verification: integration - add or update tests under `src/parallel/tests/` or `tests/` that run dry-run/parallel target selection and assert planned groups exclude unrequested changes and reject unknown IDs)
- [x] Preserve legacy `--change a,b` behavior for valid known IDs while routing it through the new explicit-target normalization. (verification: integration - keep existing `src/cli.rs` `--change` parser tests passing and add a regression test that `--change` shares duplicate/unknown validation with positional IDs)
- [x] Update CLI help text and command examples in `src/cli.rs` to require `cflx run --all` or `cflx run <change-id>...` instead of bare `cflx run`. (verification: integration - source path `src/cli.rs`; command `cargo run -- run --help` output shows explicit target examples and does not recommend bare `cflx run`)
- [x] Update bundled skill docs under `skills/cflx-run/SKILL.md`, `skills/cflx-run/references/cflx-run.md`, and `skills/README.md` to explain explicit target mode, TUI selection equivalence, legacy `--change`, and no bare `cflx run` default. (verification: integration - source paths `skills/cflx-run/SKILL.md`, `skills/cflx-run/references/cflx-run.md`, and `skills/README.md`; command `rg "cflx run" skills/cflx-run skills/README.md` shows no stale bare-run guidance after the docs update)
- [x] Run repository validation for the changed behavior and docs. (verification: integration - `cargo test`, relevant run-mode tests, and `cflx openspec validate add-run-explicit-targets --strict --evidence warn` succeed)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-run-explicit-targets --archive-gate`

## Future Work

- Consider removing `--change` in a separate deprecation proposal only if operators no longer need the legacy syntax.
