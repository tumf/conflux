---
agent: build
description: Scaffold a new OpenSpec changes and validate strictly.
---
load skill: cflx-proposal

Before processing <UserRequest>, proactively gather relevant context from the current session and repo, and treat it as the premise for the proposal. This includes (when available):
- The user's prior messages and stated goals/constraints
- Repository-specific agent instructions (e.g. AGENTS.md, openspec/AGENTS.md)
- Any already-mentioned architecture, modules, workflows, or conventions

Write a brief "Premise / Context" section first (concise bullets), then proceed to the proposal flow using that premise.

For any required `change_id`: generate it yourself from the request content (short, descriptive slug) and ensure it is unique. Do NOT ask the user to confirm or choose a `change_id`. If a collision is possible, disambiguate automatically (e.g., add a short numeric or hash suffix).

The user has requested the following change proposals.
<UserRequest>
$ARGUMENTS
</UserRequest>
