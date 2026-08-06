## Implementation Tasks

- [ ] Update `.opencode/commands/cflx-accept.md` so FAIL handling is explicitly read-only: reviewers return all actionable findings, do not edit `tasks.md` or `## Current Acceptance Follow-up`, and delegate normalized follow-up persistence to Conflux runtime. Replace external-dependency instructions to output "follow-up tasks" with instructions to output actionable findings, while preserving verdict, blocker, scoped-review, dirty-tree, behavior-task adequacy, external-dependency, and permission-error semantics. Completion requires the obsolete direct-edit, attempt-number, and numbered-section imperatives to be absent from the tracked adapter. (verification: unit - `cargo test --lib embedded_skills::tests::test_opencode_acceptance_command_is_read_only`; verification-id: opencode-acceptance-read-only-contract)

- [ ] Add `test_opencode_acceptance_command_is_read_only` in `src/embedded_skills.rs` against `CFLX_ACCEPT_COMMAND_MD`. Require explicit read-only/runtime-persistence wording and reject exactly `After listing all findings, update openspec/changes/<change_id>/tasks.md`, `Determine the next acceptance attempt number`, and `Append or create the section for that attempt`; do not reject broad substrings that can occur in legitimate prohibition text, and retain the existing behavior-task adequacy adapter test. Completion requires the named filter to select exactly one runnable test, the test source to assert all positive and exact forbidden anchors, and the current adapter test to pass without mutating the reviewed worktree. (verification: unit - `cargo test --lib embedded_skills::tests::test_opencode_acceptance_command_is_read_only`; verification-id: opencode-acceptance-read-only-contract)

## Final Validation

Archive validation is the authoritative final OpenSpec gate. Expected archive gate: `cflx openspec validate fix-opencode-acceptance-read-only --archive-gate`.

## Future Work

- Consider shared generation for runtime-specific command adapters only if additional semantic drift recurs across multiple adapters.
