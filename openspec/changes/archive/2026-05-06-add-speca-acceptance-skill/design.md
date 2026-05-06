# Design: SPECA-oriented acceptance skill

## Current Acceptance Ownership

Conflux acceptance has three relevant surfaces:

1. `.opencode/commands/cflx-accept.md` — fixed acceptance procedure and final verdict contract.
2. `skills/cflx-accept/SKILL.md` — operation identity and scoped guidance.
3. Rust prompt builders — variable context only, including change id, paths, diff, history, and selected skill prelude.

The new `cflx-accept-with-speca` skill must fit into this split. It adds a review strategy, not a new acceptance protocol.

## Skill Responsibilities

`cflx-accept-with-speca` should guide the agent through this loop:

1. **Load baseline acceptance context**
   - Read the change proposal/tasks/spec deltas.
   - Read `openspec/CONSTITUTION.md` when present.
   - Treat the standard acceptance contract as authoritative.

2. **Derive properties**
   - Convert requirements, scenarios, task claims, and changed code responsibilities into checkable properties.
   - Prefer properties that would reveal a mismatch between the OpenSpec delta and implementation behavior.
   - Keep properties tied to repository paths/functions so findings are actionable.

3. **Attempt falsification / proof**
   - Use local tests, static inspection, command output, and changed-file analysis.
   - If SPECA tooling is installed and usable, run it or use its generated proof-attempt structure.
   - If tooling is unavailable, perform a structured manual property review and say that the external tool was unavailable only in the human-readable reasoning, not in the final verdict format.

4. **Classify outcomes**
   - Blocking: concrete property failure with repository evidence; maps to `fail`.
   - Advisory: non-blocking risk or improvement; include in reasoning, do not force fail by itself.
   - Incomplete: more repository work/checks are needed; maps to `fail` when autonomously resolvable, or `gated` only under the existing blocker rubric.
   - Gated: only when repository-only work cannot resolve the issue under the standard acceptance rubric.

5. **Emit one Conflux verdict**
   - Final machine-readable output remains the standard JSON acceptance verdict, with legacy marker during rollout if the command template requires it.

## Boundary Rules

- Do not copy the command template's full checklist or formatting rules into the skill.
- Do not introduce a `SPECA: PASS/FAIL` terminal marker.
- Do not ask the user questions; acceptance is autonomous.
- Do not use external logs/caches as authoritative workflow-control inputs.
- Do not treat unavailable SPECA tooling as PASS. Use repository evidence and structured fallback review.

## Verification Strategy

- Embedded inventory test for `cflx-accept-with-speca`.
- Frontmatter/name test for the new skill.
- Drift-detection test that scans the skill for command-template-only fixed procedure phrases.
- Manual review of `SKILL.md` to ensure property/proof-attempt mapping is present and final verdict ownership remains with standard acceptance.
- Targeted `cargo test embedded_skills` after embedding.
