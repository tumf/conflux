## Implementation Tasks

- [ ] Skip AI resolve for conflictless sequential merges. Completion condition: the sequential merge path checks repository-visible conflict evidence before launching `cflx-resolve`, and does not emit `ResolveStarted` when Git reports no unresolved conflicts. (verification: integration - add/extend a focused test in `src/parallel/tests/executor.rs` or equivalent that creates an archived merge-ready change with no conflicts and asserts no resolve command/event is produced)

- [ ] Preserve AI resolve for real conflicts. Completion condition: when the sequential merge path has actual unresolved conflicts, Conflux still emits `ResolveStarted`, includes real conflict evidence, and routes through the existing resolve command path. (verification: integration - extend an existing conflict test in `src/parallel/tests/executor.rs` or `src/parallel/tests/conflict.rs` to assert `ResolveStarted` and non-empty conflict evidence for a true conflict case)

- [ ] Fix false post-merge pre-sync retry. Completion condition: post-merge verification accepts a valid merge commit for an archived change without re-retrying solely because the archived source branch tip does not contain the pre-merge base after integration. (verification: integration - add a regression test around `src/parallel/conflict.rs` / `src/parallel/tests/executor.rs` that reproduces the `Worktree branch ... does not include pre-merge base ... retrying resolve` false negative and proves it no longer retries after a successful merge commit)

- [ ] Keep resolve prompts and worktree evidence truthful. Completion condition: the conflictless path no longer builds a conflict-oriented prompt with `Conflicting files: (none)` or `(unknown)` worktree locations, while the true-conflict path still provides accurate worktree/conflict context. (verification: unit/integration - assert emitted resolve command text or `ResolveStarted` payloads from tests in `src/parallel/tests/executor.rs` or `src/parallel/tests/conflict.rs`)

- [ ] Run required Rust and proposal checks. Completion condition: formatting and targeted tests pass, and the proposal remains strictly valid. (verification: integration - `cargo fmt --check`; targeted `cargo test` for affected parallel/conflict tests; `cflx openspec validate fix-conflictless-merge-resolve-retry --strict`)

## Future Work

- If archive-to-merge verification needs a broader redesign of merge provenance rules, create a follow-up proposal instead of widening this bugfix beyond conflictless handoff correctness.
