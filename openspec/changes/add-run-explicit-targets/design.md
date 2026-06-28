# Design: explicit `cflx run` targets

## Target model

`cflx run` gets a single normalized target mode before orchestration starts:

- `All`: requested by `--all`; equivalent to marking all eligible rows with `x` in the TUI.
- `Explicit(Vec<String>)`: requested by positional IDs or legacy `--change`; equivalent to starting the TUI with exactly those rows selected.

Bare `cflx run` has no target mode and must fail before orchestration starts.

## Validation boundary

Target validation should happen against the initial OpenSpec change snapshot captured at run start. Explicit target validation must be atomic:

- duplicate ID => fail
- unknown ID => fail
- mixed known/unknown IDs => fail with no partial execution

This keeps run-mode behavior predictable and satisfies the constitution's truthful-completion rule by relying only on repository-visible change state.

## Serial, parallel, and dry-run consistency

The existing serial path already has a snapshot filtering concept. The implementation should make that filtering reusable for parallel and dry-run paths rather than adding a second target interpretation. The same filtered snapshot should initialize shared orchestration state and feed `ParallelRunService` planning/execution.

## Skill documentation

`skills/cflx-run` is operator-facing and bundled by `cflx install-skills`. It should describe the same explicit target contract that the CLI enforces, including:

- `cflx run <change-id>...` for selected changes
- `cflx run --all` for all changes
- `cflx run --change a,b` as legacy-compatible syntax
- bare `cflx run` as invalid/non-recommended
