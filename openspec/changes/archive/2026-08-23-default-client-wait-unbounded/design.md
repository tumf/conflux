# Design

## Timeout representation

Represent the operation deadline as optional rather than using a sentinel `Duration::ZERO` after parsing. The CLI accepts `0` as the user-facing sentinel, but wait internals must make bounded and unbounded control flow explicit.

## Deadline behavior

For a positive timeout, preserve the current single monotonic operation deadline across observation, event recovery, repository classification, and child Git work.

For zero or omission, do not create an overall operation deadline. The per-request transport valve (`EXCHANGE_TIMEOUT` in `src/client/transport.rs`) already bounds each owner exchange independently of the operation deadline and remains in force. Git subprocesses are different: today the operation deadline is the *only* bound that reaches a Git child — `run_git` with a `None` deadline runs the child unbounded (`src/bounded_git.rs`), and `ls-remote` against an unresponsive remote can hang indefinitely (`src/client/repo.rs`). An unbounded wait therefore must introduce a finite per-invocation deadline for every Git subprocess it spawns rather than passing no deadline down. Expiry of that inner deadline terminates and reaps the child and is handled as a recoverable retry or a typed evidence condition; it must not be translated into the operation-level `timeout` outcome reserved for explicit positive deadlines.

## Compatibility

Positive timeout syntax and outcomes are unchanged. Zero changes from a parse error to the documented unbounded sentinel, and every accepted spelling whose value is exactly zero (`0`, `0s`, `0ms`, `0m`, `0h`) selects it uniformly — the existing usage-rejection test for `--timeout 0s` moves to asserting the sentinel. Positive values below the existing minimum remain usage errors. Completion evidence and all mutation prohibitions remain unchanged.
