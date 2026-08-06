## Implementation Tasks

- [ ] Update `.opencode/commands/cflx-accept.md` so FAIL handling is explicitly read-only: reviewers return all actionable findings, do not edit `tasks.md` or `## Current Acceptance Follow-up`, and delegate normalized follow-up persistence to Conflux runtime. Preserve all unrelated review checks and verdict guidance. Completion requires the obsolete direct-edit, attempt-number, and numbered-section instructions to be absent from the tracked adapter. (verification: unit - `cargo test --lib embedded_skills::tests::test_opencode_acceptance_command_is_read_only`; verification-id: opencode-acceptance-read-only-contract)

- [ ] Add `test_opencode_acceptance_command_is_read_only` in `src/embedded_skills.rs` against `CFLX_ACCEPT_COMMAND_MD`. Require positive read-only/runtime-owner wording and reject instructions to update `tasks.md`, determine a next Acceptance attempt number, or append numbered Failure Follow-up sections; retain the existing behavior-task adequacy adapter test. Completion requires the test to select exactly one runnable test and to fail when the stale command tail is restored. (verification: unit - `cargo test --lib embedded_skills::tests::test_opencode_acceptance_command_is_read_only`; verification-id: opencode-acceptance-read-only-contract)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-opencode-acceptance-read-only --archive-gate`.

## Future Work

- Consider shared generation for runtime-specific command adapters only if additional semantic drift recurs across multiple adapters.
