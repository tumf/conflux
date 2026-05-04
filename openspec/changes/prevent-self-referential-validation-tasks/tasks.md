## Implementation Tasks

- [ ] Add native validator detection for self-referential final validation checkbox tasks. (verification: unit - add tests in `src/openspec_cmd.rs` around `OpenSpecManager::validate_change` / task evidence validation covering a `tasks.md` checkbox that runs `cflx openspec validate <same-change>`; completion condition: `cflx openspec validate <fixture-change> --strict --evidence error` fails with a diagnostic that names self-referential final validation)

- [ ] Allow non-checkbox `## Final Validation` sections to mention the same validation command. (verification: unit - add a fixture/test in `src/openspec_cmd.rs` where `tasks.md` has `## Final Validation` text with `cflx openspec validate <same-change> --strict --evidence warn` but no checkbox; completion condition: validation passes and no self-referential final validation diagnostic is emitted)

- [ ] Replace generic evidence-note diagnostics for this pattern with a targeted remediation message. (verification: unit - assert diagnostics from `src/openspec_cmd.rs` contain guidance equivalent to `move final validation to a non-checkbox Final Validation section` instead of only `Verification note should cite repository-verifiable evidence`; completion condition: the targeted diagnostic is returned for checkbox self-reference)

- [ ] Update bundled `cflx-proposal` guidance/templates so final OpenSpec validation is not generated as an implementation checkbox task. (verification: unit - add/update repository text validation covering `skills/cflx-proposal/SKILL.md` and/or embedded skill source paths under `skills/`; completion condition: running `cargo test openspec_cmd --lib` plus the new text test proves final OpenSpec validation guidance uses a non-checkbox `## Final Validation` section or archive-gate note instead of `- [ ]`)

- [ ] Update archive-side guidance/failure reporting to surface archive-equivalent validation. (verification: unit - add/update tests for archive prompt/error construction in `skills/cflx-archive/SKILL.md`, `src/execution/archive.rs`, or the archive command path that preserves root cause; completion condition: archive guidance tells users to run the archive-equivalent validator and explicitly names self-referential final validation checkbox when detected)

- [ ] Provide a reproducible archive-gate validation command. (verification: integration - either add `cflx openspec validate <change-id> --archive-gate` with CLI parsing/tests in `src/cli.rs`, `src/main.rs`, and `src/openspec_cmd.rs`, or document/enforce `cflx openspec validate <change-id> --strict --evidence error` as the exact archive readiness check; completion condition: tests prove the local command fails for evidence warnings in the same way archive readiness does)

- [ ] Preserve ordinary evidence validation behavior. (verification: unit - run/update existing `src/openspec_cmd.rs` tests for evidence hints and add self-reference tests; completion condition: ordinary repository-verifiable task notes such as `cargo test`, `npm run`, `go test`, and source/test file paths still pass)

- [ ] Run targeted validation and tests. (verification: integration - run targeted commands such as `cargo test openspec_cmd --lib`, any archive prompt/error test filters added for this change, and `cflx openspec validate prevent-self-referential-validation-tasks --strict --evidence warn`; completion condition: all commands exit 0 and this proposal has no evidence warnings)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate. Keep this section non-checkbox to avoid recreating the self-referential task pattern this change prevents.

Expected archive gate: `cflx openspec validate prevent-self-referential-validation-tasks --strict --evidence warn` exits 0 with no evidence warnings before archive promotion.

## Future Work

- Consider a separate cleanup change if historical archived task files should be normalized for documentation consistency.
