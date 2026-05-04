## Implementation Tasks

- [x] Add native validator detection for self-referential final validation checkbox tasks. (verification: unit - `src/openspec_cmd.rs` adds `is_self_referential_final_validation_task` and `test_rejects_self_referential_final_validation_checkbox` covering a `tasks.md` checkbox that runs `cflx openspec validate <same-change>`; completion condition: validation with strict evidence errors fails with a diagnostic that names self-referential final validation)

- [x] Allow non-checkbox `## Final Validation` sections to mention the same validation command. (verification: unit - `test_allows_non_checkbox_final_validation_section` in `src/openspec_cmd.rs` covers `## Final Validation` text with `cflx openspec validate <same-change> --strict --evidence warn` but no checkbox; completion condition: validation passes and no self-referential final validation diagnostic is emitted)

- [x] Replace generic evidence-note diagnostics for this pattern with a targeted remediation message. (verification: unit - `test_rejects_self_referential_final_validation_checkbox` in `src/openspec_cmd.rs` asserts diagnostics contain guidance equivalent to `move final validation to a non-checkbox Final Validation section` instead of only `Verification note should cite repository-verifiable evidence`; completion condition: the targeted diagnostic is returned for checkbox self-reference)

- [x] Update bundled `cflx-proposal` guidance/templates so final OpenSpec validation is not generated as an implementation checkbox task. (verification: unit - `skills/cflx-proposal/SKILL.md` now instructs non-checkbox `## Final Validation` with `--archive-gate`, and `test_cflx_proposal_skill_final_validation_uses_non_checkbox_section` covers the text; completion condition: running `cargo test openspec_cmd --lib` plus the new text test proves final OpenSpec validation guidance uses a non-checkbox `## Final Validation` section or archive-gate note instead of `- [ ]`)

- [x] Update archive-side guidance/failure reporting to surface archive-equivalent validation. (verification: unit - `skills/cflx-archive/SKILL.md` documents `cflx openspec validate <id> --archive-gate`, and `src/execution/archive.rs` tests `test_build_archive_error_message` / `test_build_archive_error_message_names_self_referential_validation_task` cover archive error construction that preserves root cause; completion condition: archive guidance tells users to run the archive-equivalent validator and explicitly names self-referential final validation checkbox when detected)

- [x] Provide a reproducible archive-gate validation command. (verification: integration - `src/cli.rs` adds `--archive-gate`, `src/main.rs` maps it to strict validation with evidence `error`, and `test_openspec_validate_archive_gate_flag` covers CLI parsing; completion condition: tests prove the local command fails for evidence warnings in the same way archive readiness does)

- [x] Preserve ordinary evidence validation behavior. (verification: unit - `src/openspec_cmd.rs` retains existing evidence hint tests and adds `test_preserves_ordinary_repository_evidence`; completion condition: ordinary repository-verifiable task notes such as `cargo test`, `npm run`, `go test`, and source/test file paths still pass)

- [x] Run targeted validation and tests. (verification: integration - ran `cargo test openspec_cmd --lib`, `cargo test test_openspec_validate_archive_gate_flag --lib`, `cargo test test_build_archive_error_message --lib`, `cargo fmt --all -- --check`, and `cflx openspec validate prevent-self-referential-validation-tasks --strict --evidence warn`; completion condition: all commands exit 0 and this proposal has no evidence warnings)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate. Keep this section non-checkbox to avoid recreating the self-referential task pattern this change prevents.

Expected archive gate: `cflx openspec validate prevent-self-referential-validation-tasks --strict --evidence warn` exits 0 with no evidence warnings before archive promotion.

## Future Work

- Consider a separate cleanup change if historical archived task files should be normalized for documentation consistency.
