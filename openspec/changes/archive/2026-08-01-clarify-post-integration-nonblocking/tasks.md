## Implementation Tasks

- [x] Update `skills/cflx-accept/SKILL.md` `### Declared Verification Phases` section: replace the unconditional "non-mockable external prerequisite that makes declared automation unusable is a stalled hold" with `completion_role`-aware rubric (verification: unit - `cargo test --lib embedded_skills` in src/embedded_skills.rs asserts the embedded rubric sentence is scoped to `completion_role: change-blocking`; verification-id: skill-post-integration-nonblocking)
- [x] Add a new subsection `#### Completion Role Gating` under `### Declared Verification Phases` with an explicit decision table: `completion_role: change-blocking` + missing prerequisite → stalled hold; `completion_role: operational-observation` + any state → acknowledge as pending, PASS or FAIL on other grounds only (verification: unit - `cargo test --lib embedded_skills` in src/embedded_skills.rs asserts the `#### Completion Role Gating` heading and the unavailable-prerequisite decision-table row; verification-id: skill-post-integration-nonblocking)
- [x] Add a concrete example scenario: post-integration physical-device scan with unavailable prerequisite → acknowledge as pending, emit PASS if all pre-integration verifications pass (verification: unit - `cargo test --lib embedded_skills` in src/embedded_skills.rs asserts the `#### Example: post-integration physical-device scan` heading is embedded; verification-id: skill-post-integration-nonblocking)
- [x] Mirror the `completion_role` scoping into `skills/cflx-accept-with-speca/SKILL.md`, which duplicates the same unconditional stalled-hold sentence while declaring it applies "the same structured verification-phase semantics as `cflx-accept`" (verification: unit - `cargo test --lib embedded_skills`; verification-id: skill-post-integration-nonblocking)
- [x] Add unit test `acceptance_skills_scope_stalled_holds_to_change_blocking_verifications` in `src/embedded_skills.rs` asserting both embedded acceptance skills scope the hold to `change-blocking`, no longer carry the unconditional sentence, and that `cflx-accept` carries the decision table and example (verification: unit - `cargo test --lib embedded_skills` 31 passed; verification-id: skill-post-integration-nonblocking)
- [x] Update `openspec/specs/agent-prompts/spec.md`: MODIFY `Requirement: Acceptance MUST honor declared verification phases` — scope stalled-hold eligibility to `completion_role: change-blocking` only; add scenario for operational-observation non-blocking (verification: unit - runnable command `cflx openspec validate clarify-post-integration-nonblocking --strict`; verification-id: skill-post-integration-nonblocking) — carried by the change delta `specs/agent-prompts/spec.md`, which archive applies to the canonical spec
- [x] Verify `cflx install-skills` embeds the updated skill into the binary correctly: compile and run `cflx install-skills`, confirm the installed `cflx-accept/SKILL.md` has the new content (verification: manual - runnable command `cargo build --bin cflx` then `cflx install-skills` inside a throwaway temp dir, then `diff <tmp>/.agents/skills/cflx-accept/SKILL.md skills/cflx-accept/SKILL.md`; verification-id: skill-post-integration-nonblocking) — ran project scope in a throwaway temp dir instead of `--global`; run record in `## Notes`

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate clarify-post-integration-nonblocking --archive-gate`

## Notes

Task 9 run record (`cflx install-skills` embedding check): built the worktree binary with
`cargo build --bin cflx`, ran `cflx install-skills` (project scope) inside a `mktemp -d` directory
instead of `--global` so the developer's `~/.agents/skills` is not overwritten, then diffed the
installed files against the sources. Result: `Successfully installed 12 skill(s).`, and both
`<tmp>/.agents/skills/cflx-accept/SKILL.md` and `<tmp>/.agents/skills/cflx-accept-with-speca/SKILL.md`
are byte-identical to `skills/cflx-accept/SKILL.md` and `skills/cflx-accept-with-speca/SKILL.md`,
with the installed `cflx-accept/SKILL.md` carrying the new `#### Completion Role Gating` section.
