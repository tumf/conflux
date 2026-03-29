---
agent: build
description: Scaffold a new OpenSpec changes and validate strictly.
---
load skill: cflx-proposal

Before processing <UserRequest>, proactively gather relevant context from the current session and repo, and treat it as the premise for the proposal. This includes (when available):
- The user's prior messages and stated goals/constraints
- Repository-specific agent instructions (e.g. AGENTS.md, openspec/AGENTS.md)
- Any already-mentioned architecture, modules, workflows, or conventions

If <UserRequest> is empty or only whitespace, do NOT stop. Instead, reconstruct the effective request from the current session and repository context gathered above.

Priority order for deriving the effective request:
1. Explicit <UserRequest> content
2. The user's most recent actionable request in this session
3. The immediately preceding discussion about a desired change, proposal, spec, or behavior change
4. Repo-specific conventions or constraints that clarify the intended proposal scope

When operating without explicit <UserRequest>:
- Write a brief "Premise / Context" section first (concise bullets)
- State the inferred request in 1-3 bullets under that section
- Proceed to the proposal flow using that inferred request
- Only decline to create a proposal if no concrete actionable change can be inferred from session context

For any required `change_id`: generate it yourself from the request content (short, descriptive slug) and ensure it is unique. Do NOT ask the user to confirm or choose a `change_id`. If a collision is possible, disambiguate automatically (e.g., add a short numeric or hash suffix).

The user has requested the following change proposals.
<UserRequest>
$ARGUMENTS
</UserRequest>

If the UserRequest block is empty, treat the effective request as inferred from the current session context rather than as missing input.

After successfully creating and strictly validating the proposal, stage the proposal-related changes and create a git commit for them. Do not stop after authoring the proposal if the proposal was created successfully.
