## Implementation Tasks

- [ ] Update `skills/cflx-accept/SKILL.md` `### Declared Verification Phases` section: replace the unconditional "non-mockable external prerequisite that makes declared automation unusable is a stalled hold" with `completion_role`-aware rubric (verification: unit - manual diff review of skill file; verification-id: skill-post-integration-nonblocking)
- [ ] Add a new subsection `#### Completion Role Gating` under `### Declared Verification Phases` with an explicit decision table: `completion_role: change-blocking` + missing prerequisite → stalled hold; `completion_role: operational-observation` + any state → acknowledge as pending, PASS or FAIL on other grounds only (verification: unit - manual diff review; verification-id: skill-post-integration-nonblocking)
- [ ] Add a concrete example scenario: post-integration physical-device scan with unavailable prerequisite → acknowledge as pending, emit PASS if all pre-integration verifications pass (verification: unit - manual diff review; verification-id: skill-post-integration-nonblocking)
- [ ] Update `openspec/specs/agent-prompts/spec.md`: MODIFY `Requirement: Acceptance MUST honor declared verification phases` — scope stalled-hold eligibility to `completion_role: change-blocking` only; add scenario for operational-observation non-blocking (verification: unit - cflx openspec validate clarify-post-integration-nonblocking --strict; verification-id: skill-post-integration-nonblocking)
- [ ] Verify `cflx install-skills` embeds the updated skill into the binary correctly: compile and run `cflx install-skills --global`, confirm `~/.agents/skills/cflx-accept/SKILL.md` has the new content (verification: manual - install skills and diff; verification-id: skill-post-integration-nonblocking)

## Final Validation

Archive validation is the authoritative final OpenSpec gate.
Expected archive gate: `cflx openspec validate clarify-post-integration-nonblocking --archive-gate`
