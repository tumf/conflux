## Implementation Tasks

- [x] Add canonical target validation to strict OpenSpec proposal validation. Completion condition: `src/openspec_cmd.rs` validates every `## MODIFIED Requirements` and `## REMOVED Requirements` block against the corresponding `openspec/specs/<capability>/spec.md` canonical requirement headings during `cflx openspec validate <change-id> --strict`, while leaving `ADDED Requirements` unchanged. (verification: unit - add focused tests in `src/openspec_cmd.rs` covering valid modified target, missing modified target, missing removed target, and added-only delta behavior)

- [x] Reuse archive promotion target-matching semantics for validator diagnostics. Completion condition: strict validation diagnostics for missing `MODIFIED` or `REMOVED` targets use the same normalized requirement identity behavior and actionable wording as archive promotion errors, including the capability name and missing `### Requirement:` heading. (verification: unit - add assertions in `src/openspec_cmd.rs` tests that diagnostics contain `MODIFIED target not found in canonical spec` and `REMOVED target not found in canonical spec` for fixture deltas)

- [x] Ensure archive-gate validation reports canonical target blockers before archive command execution. Completion condition: `cflx openspec validate <change-id> --archive-gate` fails on missing canonical `MODIFIED`/`REMOVED` targets without requiring `cflx openspec archive` to run. (verification: integration - add or update tests covering the archive-gate mode path in `src/openspec_cmd.rs`, or document an equivalent local fixture command whose output contains the missing-target diagnostic)

- [x] Update bundled `cflx-proposal` guidance for spec delta authoring. Completion condition: `skills/cflx-proposal/SKILL.md` tells proposal authors to inspect canonical `openspec/specs/<capability>/spec.md` requirement headings before selecting `MODIFIED` or `REMOVED`, to use `ADDED` when no canonical heading exists, and to run strict validation after authoring. (verification: manual - inspect source path `skills/cflx-proposal/SKILL.md`, add or update `src/templates.rs`/skill embedding tests if present, and run `cflx openspec validate <id> --strict`)

- [x] Preserve existing validator behavior for no-delta and ordinary evidence checks. Completion condition: existing strict validation rules for `specs/.no-delta`, missing spec deltas, scenario presence, and task verification evidence continue to pass their current tests after target validation is added. (verification: unit - run `cargo test openspec_cmd --lib` or targeted validator filters that include no-delta, evidence, and spec delta validation tests)

- [x] Run final repository verification for the changed areas. Completion condition: formatting and relevant tests pass without introducing default-suite heavy tests over 1 second unless marked `heavy`. (verification: integration - run `cargo fmt --check`, `cargo test openspec_cmd --lib`, and `cflx openspec validate fix-spec-delta-target-validation --strict --evidence warn`)

## Future Work

- Consider a later UX improvement that suggests the closest existing canonical requirement headings when a `MODIFIED` or `REMOVED` target is missing.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate fix-spec-delta-target-validation --archive-gate`
