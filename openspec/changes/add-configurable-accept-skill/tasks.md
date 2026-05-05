## Implementation Tasks

- [ ] Task 1: Add `accept_skill` to configuration loading and merging. (verification: unit - add or update config tests in `src/config/types.rs`, `src/config/load.rs`, or adjacent config test modules covering omitted default, project/global/custom override precedence, and accessor returning `cflx-accept` when unset; completion condition: tests fail without the new field and pass with the implemented config behavior)
- [ ] Task 2: Thread the configured acceptance skill into prompt construction. (verification: unit - update `src/agent/prompt.rs` tests for `build_acceptance_prompt` / context-only prompt generation to assert default `load skills: cflx-accept` and custom `load skills: cflx-accept-with-speca` while preserving existing section order; completion condition: hardcoded `load skills: cflx-accept` is no longer the only possible prelude)
- [ ] Task 3: Preserve acceptance execution and verdict behavior. (verification: integration - run targeted acceptance tests such as `cargo test acceptance` plus prompt tests touching `src/orchestration/acceptance.rs` and `src/agent/prompt.rs`, confirming command execution and JSON verdict parsing are unchanged by skill selection; completion condition: existing acceptance verdict tests still pass)
- [ ] Task 4: Add bundled `cflx-accept-with-speca` skill. (verification: unit/manual - add `skills/cflx-accept-with-speca/SKILL.md`, register it in `src/embedded_skills.rs`, and add/extend embedded-skill tests proving the built-in skill is exposed; completion condition: built-in skill list includes both `cflx-accept` and `cflx-accept-with-speca`)
- [ ] Task 5: Define SPECA acceptance guidance in `skills/cflx-accept-with-speca/SKILL.md`. (verification: manual - inspect `skills/cflx-accept-with-speca/SKILL.md`, `skills/cflx-accept/SKILL.md`, and `.opencode/commands/cflx-accept.md`, then run `cargo test embedded_skills` or the equivalent embedded-skill test target to confirm the bundled skill remains loadable; completion condition: the skill preserves the same JSON verdict contract, delegates fixed formatting rules to the standard acceptance contract, does not introduce a second incompatible output format, and explains property/proof-attempt behavior plus final verdict mapping)
- [ ] Task 6: Document the config option and opt-in example. (verification: manual - update `src/templates.rs` and any relevant docs/templates, then inspect those files for `accept_skill`, `cflx-accept`, and `cflx-accept-with-speca` examples so users can discover the option; completion condition: templates or docs mention default `cflx-accept` and the SPECA opt-in example)
- [ ] Task 7: Run formatting and targeted verification. (verification: manual - run `cargo fmt --check`, config tests, prompt tests, embedded skill tests, and targeted acceptance tests such as `cargo test acceptance`; completion condition: commands pass or any long-running checks are classified according to repository test policy)

## Future Work

- First-class `speca-accept` adapter binary and prompt-file/stdin handoff remain separate follow-up work.
- Structured dashboard display of SPECA properties, proof traces, and finding metadata remains separate follow-up work.

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate add-configurable-accept-skill --archive-gate`
