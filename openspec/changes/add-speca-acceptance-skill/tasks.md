## Implementation Tasks

- [x] Task 1: Add `skills/cflx-accept-with-speca/SKILL.md`. (verification: unit - add `src/embedded_skills.rs` test assertions that `skills/cflx-accept-with-speca/SKILL.md` exists through compile-time embedding, has name `cflx-accept-with-speca`, includes property/proof-attempt guidance, and preserves standard verdict ownership; completion condition: the skill exists and describes SPECA-style review without redefining the final verdict protocol)
- [x] Task 2: Embed the new bundled skill. (verification: unit - update `src/embedded_skills.rs` and run `cargo test embedded_skills`; completion condition: the built-in skill list exposes both `cflx-accept` and `cflx-accept-with-speca`)
- [x] Task 3: Add ownership drift tests for the SPECA skill. (verification: unit - extend tests in `src/embedded_skills.rs` so command-template-only fixed procedure phrases are absent from `cflx-accept-with-speca` and the skill references `.opencode/commands/cflx-accept.md` or the standard acceptance contract; completion condition: tests fail if the skill copies fixed formatting/checklist ownership from `.opencode/commands/cflx-accept.md`)
- [x] Task 4: Document opt-in usage. (verification: manual - update `src/templates.rs` or repository docs if `accept_skill` is available, and inspect for `"accept_skill": "cflx-accept-with-speca"`; completion condition: users can discover how to select the SPECA acceptance skill through config)
- [x] Task 4.1: Complete `accept_skill` configuration plumbing required by the SPECA skill spec. (verification: unit - `src/config/types.rs` tests cover operation skill defaults and merge precedence; `src/agent/prompt.rs` test shows `accept_skill = "cflx-accept-with-speca"` changes only the `load skills:` prelude; completion condition: the SPECA skill can be selected without command replacement)
- [x] Task 5: Run targeted verification. (verification: manual - run `cargo fmt --check`, `cargo test embedded_skills`, and any config/prompt tests touched by `src/templates.rs` or opt-in documentation; completion condition: commands pass or long-running checks are classified according to repository test policy)

## Future Work

- First-class external SPECA runner integration or a `cflx speca` command remains a separate change.
- Structured persistence/display of SPECA property traces remains a separate observability/dashboard change.
- Mandatory proof-attempt execution can be considered later after the runner contract is stable.

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate add-speca-acceptance-skill --archive-gate`
