---
agent: build
description: Scaffold a new OpenSpec change and validate strictly.
---

The user has requested the following change proposal. Use the npx @fission-ai/openspec@latest instructions to create their change proposal.
<UserRequest>
$ARGUMENTS
</UserRequest>

**NOTE**:
- Always consider the preceding conversation context to interpret the user's intent. If `$ARGUMENTS` is empty, summarize the conversation conclusions and create a proposal. If a change ID is not explicitly provided but can be inferred from context, use it without asking the user.

**MUST**: Bugfixes with no intended spec changes still need at least one minimal `## MODIFIED Requirements` delta (one requirement + one `#### Scenario:`) so `npx @fission-ai/openspec@latest validate <id> --strict` passes.
**MUST**: If a task is not executable by the AI (requires human action, external systems, or long-wait verification), either move it to a Future work section or omit it from tasks.md entirely.

**External dependency policy (mock-first / verification-first)**:
- Any requirement that AI cannot resolve or verify autonomously is an external dependency
- "Mock-first" means: prioritize mocks/stubs/fixtures as a verification strategy so tasks can be validated without external credentials
- DO NOT replace production/runtime behavior with mocks/stubs to satisfy requirements; mocks/stubs/fixtures are for tests and local verification only
- Only truly non-mockable external dependencies (requiring real external systems, human decisions, or long-wait verification) should be marked for Out of Scope / Future Work
- Missing secrets (API keys, credentials) should NOT block proposal design; instead, design the interface/contract boundaries and include test-only mock/stub/fixture tasks
- When creating tasks.md, include specific tasks for mock/stub/fixture implementation where applicable

**CRITICAL RESTRICTIONS**
- This command is for PROPOSAL CREATION ONLY
- DO NOT implement or modify source code
- DO NOT edit files outside `openspec/changes/` directory
- You may READ any files for context gathering
- You may WRITE only to `openspec/changes/<id>/` paths
- After proposal validation with `npx @fission-ai/openspec@latest validate <id> --strict`, STOP and present the proposal to the user

**Guardrails**
- Favor straightforward, minimal implementations first and add complexity only when it is requested or clearly required.
- Keep changes tightly scoped to the requested outcome.
- Default to proposal splitting: when requirements can be decomposed into independent scopes, create separate change proposals.
- If uncertain whether to split, prefer splitting unless the scopes are tightly coupled and must ship together to preserve correctness.
- For each split proposal, use a distinct verb-led `change-id` and keep `proposal.md`, `tasks.md`, and `design.md` (when needed) scoped to that proposal only.
- When multiple proposals are created, explicitly document dependency/sequence relationships and parallelizability in the final user-facing summary.
- Refer to `openspec/AGENTS.md` (located inside the `openspec/` directory—run `ls openspec` or `npx @fission-ai/openspec@latest update` if you don't see it) if you need additional OpenSpec conventions or clarifications.
- Identify any vague or ambiguous details and ask the necessary follow-up questions before editing files.

**Steps**
1. Run `npx @fission-ai/openspec@latest list` and `npx @fission-ai/openspec@latest list --specs`, and inspect related code or docs (e.g., via `rg`/`ls`) to ground the proposal in current behaviour; note any gaps that require clarification.
2. Choose a unique verb-led `change-id` and scaffold `proposal.md`, `tasks.md`, and `design.md` (when needed) under `openspec/changes/<id>/`.
3. Map the request into concrete capabilities or requirements and evaluate split boundaries first (independence, ownership, rollout risk, and coupling).
   - If scopes are independent or weakly coupled, split into separate change proposals.
   - Keep as a single proposal only when changes are tightly coupled and need atomic rollout; record that rationale in `design.md` or `proposal.md`.
4. For split proposals, scaffold and draft each `openspec/changes/<id>/` package independently, including its own spec deltas and task checklist.
5. Capture architectural reasoning in `design.md` when the solution spans multiple systems, introduces new patterns, or demands trade-off discussion before committing to specs.
6. Draft spec deltas in `changes/<id>/specs/<capability>/spec.md` (one folder per capability) using `## ADDED|MODIFIED|REMOVED Requirements` with at least one `#### Scenario:` per requirement and cross-reference related capabilities when relevant.
7. Draft `tasks.md` as an ordered checklist so OpenSpec apply tracking can parse progress:
   - Use the exact format required by `npx @fission-ai/openspec@latest instructions tasks --change <change-id>`.
   - Group tasks under `## <num>. ...` headings and use `- [ ] <num.num> ...` checkboxes.
   - Include verification per task (expected file/command/output).
8. For any new capability, include explicit integration/entry-point tasks ("wire it into the execution path") and completion criteria (what code path proves it is used).
9. Each task must state how completion is verified (e.g., where it is called, the command/output that proves it, or the file/line to inspect).
10. Validate with `npx @fission-ai/openspec@latest validate <id> --strict` and resolve every issue before sharing the proposal.
    - When multiple proposals are created, run strict validation for each `change-id`.
11. In the final response, always present a proposal index when split occurred:
    - `change-id`
    - one-line objective
    - dependency/sequence (if any)
    - whether it can be implemented in parallel

**Reference**
- Use `npx @fission-ai/openspec@latest show <id> --json --deltas-only` or `npx @fission-ai/openspec@latest show <spec> --type spec` to inspect details when validation fails.
- Search existing requirements with `rg -n "Requirement:|Scenario:" openspec/specs` before writing new ones.
- Explore the codebase with `rg <keyword>`, `ls`, or direct file reads so proposals align with current implementation realities.
