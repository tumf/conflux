---
name: OpenSpec: Proposal
description: Scaffold a new OpenSpec change and validate strictly.
category: Conflux
tags: [openspec, cflx, conflux, change]
---

The user has requested the following change proposal. Use the npx @fission-ai/openspec@latest instructions to create their change proposal.

**NOTE**:
- Always consider the preceding conversation context to interpret the user's intent. If context is empty, summarize the conversation conclusions and create a proposal. If a change ID is not explicitly provided but can be inferred from context, use it without asking the user.

**MUST**: The changes/* (tasks.md, design.md, proposal.md) must be written in Japanese.
**MUST**: `proposal.md` should start with YAML frontmatter containing `change_type`, `priority`, optional `dependencies`, and optional `references`.
**MUST**: `references` is the canonical field name for related files/specs/change IDs.
**MUST**: If `dependencies` is present in frontmatter, it overrides any body `## Dependencies` section; if absent, body dependencies remain allowed for backward compatibility.
**MUST**: Bugfixes with no intended spec changes still need at least one minimal `## MODIFIED Requirements` delta (one requirement + one `#### Scenario:`) so `npx @fission-ai/openspec@latest validate <id> --strict` passes.
**MUST**: If a task is not executable by the AI (requires human action, external systems, or long-wait verification), either move it to a Future work section or omit it from tasks.md entirely.

**CRITICAL RESTRICTIONS**
- This command is for PROPOSAL CREATION ONLY
- DO NOT implement or modify source code
- DO NOT edit files outside `openspec/changes/` directory
- You may READ any files for context gathering
- You may WRITE only to `openspec/changes/<id>/` paths
- After proposal validation with `npx @fission-ai/openspec@latest validate <id> --strict`, STOP and present the proposal to the user

<!-- OPENSPEC:START -->
**Guardrails**
- Favor straightforward, minimal implementations first and add complexity only when it is requested or clearly required.
- Keep changes tightly scoped to the requested outcome.
- When user requirements can be decomposed into multiple independent proposals, actively create separate change proposals to enable parallel work.
- Refer to `openspec/AGENTS.md` (located inside the `openspec/` directory—run `ls openspec` or `npx @fission-ai/openspec@latest update` if you don't see it) if you need additional OpenSpec conventions or clarifications.
- Identify any vague or ambiguous details and ask the necessary follow-up questions before editing files.

**Steps**
1. Review `openspec/project.md`, run `npx @fission-ai/openspec@latest list` and `npx @fission-ai/openspec@latest list --specs`, and inspect related code or docs (e.g., via `rg`/`ls`) to ground the proposal in current behaviour; note any gaps that require clarification.
2. Choose a unique verb-led `change-id` and scaffold `proposal.md`, `tasks.md`, and `design.md` (when needed) under `openspec/changes/<id>/`.
3. Map the change into concrete capabilities or requirements, breaking multi-scope efforts into distinct spec deltas with clear relationships and sequencing.
4. Capture architectural reasoning in `design.md` when the solution spans multiple systems, introduces new patterns, or demands trade-off discussion before committing to specs.
5. Draft spec deltas in `changes/<id>/specs/<capability>/spec.md` (one folder per capability) using `## ADDED|MODIFIED|REMOVED Requirements` with at least one `#### Scenario:` per requirement and cross-reference related capabilities when relevant.
6. Draft `proposal.md` with YAML frontmatter first, then human-readable sections; include `references` for relevant repo paths/specs/change IDs when they help implementation or review.
7. Draft `tasks.md` as an ordered list of small, verifiable work items that deliver user-visible progress, include validation (tests, tooling), and highlight dependencies or parallelizable work.
7. For any new capability, include explicit integration/entry-point tasks ("wire it into the execution path") and completion criteria (what code path proves it is used).
8. Each task must state how completion is verified (e.g., where it is called, the command/output that proves it, or the file/line to inspect).
9. Validate with `npx @fission-ai/openspec@latest validate <id> --strict` and resolve every issue before sharing the proposal.

**Reference**
- Use `npx @fission-ai/openspec@latest show <id> --json --deltas-only` or `npx @fission-ai/openspec@latest show <spec> --type spec` to inspect details when validation fails.
- Search existing requirements with `rg -n "Requirement:|Scenario:" openspec/specs` before writing new ones.
- Explore the codebase with `rg <keyword>`, `ls`, or direct file reads so proposals align with current implementation realities.
<!-- OPENSPEC:END -->
