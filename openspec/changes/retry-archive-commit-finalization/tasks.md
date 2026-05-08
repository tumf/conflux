## Implementation Tasks

- [x] Task 1: Refactor `src/execution/archive.rs::ensure_archive_commit` into a bounded finalization attempt loop that repeatedly checks `is_archive_commit_complete`, runs direct commit when changes are staged/dirty, runs archive-finalization resolve when direct commit fails or verification remains incomplete, and exits only on success or retry exhaustion. (verification: unit - add archive tests in `src/execution/archive.rs` proving a first failed commit/final verification can be retried and succeeds on a later attempt)
- [x] Task 2: Preserve and inject prior archive-finalization failure context into subsequent resolve prompts, including direct commit stderr, resolve exit status, stdout/stderr tail, `git status --porcelain`, and final `is_archive_commit_complete` state. (verification: unit - test prompt/context construction includes a previous hook or clippy stderr sample and the current dirty/clean archive state before the next resolve attempt)
- [x] Task 3: Update `src/parallel/executor.rs` archive flow so archive move verification success is not treated as sufficient when final archive commit creation fails; finalization failures must be retried through the new bounded loop before returning `Archive commit verification failed`. (verification: integration - parallel archive test simulates archive files moved plus failed archive commit hook on first attempt, then a successful retry without re-running the full archive command unnecessarily)
- [x] Task 4: Add user-visible retry telemetry/events/logging for archive commit finalization retries, distinguishing them from archive command retries and preserving the last actionable blocker in the exhausted error. (verification: integration - event tests assert a finalization retry log/event is emitted after a commit hook failure and that exhausted retries report the finalization phase plus last stderr)
- [x] Task 5: Ensure pre-commit hook file modifications remain repairable by re-staging and retrying rather than stopping after the first modified-file hook result. (verification: integration - git fixture test uses a hook that modifies a file on first commit attempt and verifies Conflux stages the modification and completes `Archive: <change_id>` on retry)
- [x] Task 6: Add regression coverage for the observed missing-binary-module case by simulating a clippy/pre-commit failure that is fixed during a later archive-finalization resolve attempt. (verification: integration - archive finalization test asserts a first stderr containing `could not find dependency_targets in the crate root` is passed to the retry context and that the next attempt can complete)
- [x] Task 7: Run targeted Rust verification for archive finalization and parallel archive behavior. (verification: integration - run `cargo test execution::archive --lib` and the relevant `parallel::tests::executor` archive-finalization regression tests)

## Future Work

- Consider making the archive commit finalization retry budget configurable if operators need different retry depth for very slow or expensive hooks.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate retry-archive-commit-finalization --archive-gate`
