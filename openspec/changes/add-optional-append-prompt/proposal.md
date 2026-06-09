---
change_type: implementation
priority: medium
dependencies: []
references:
  - "openspec/specs/configuration/spec.md"
  - "src/config/types.rs"
  - "src/agent/prompt.rs"
---

# Change: add optional append prompts

**Change Type**: implementation

## Problem

Conflux builds operation prompts for agent commands such as `apply_command`, `acceptance_command`, `archive_command`, `analyze_command`, and `resolve_command`. These prompts encode Conflux workflow contracts, but users currently have no generic configuration surface for appending environment-specific guidance.

A concrete example is advising acceptance agents to optionally use local review tools such as `ocr review` as advisory evidence only. Such tools may be unavailable or misconfigured in a user's environment, and their findings can be wrong when they lack OpenSpec context, so Conflux should not hard-code them into commands or treat them as gates.

## Proposed Solution

Add top-level, optional operation-specific append prompt fields to `OrchestratorConfig`:

- `apply_append_prompt`
- `acceptance_append_prompt`
- `archive_append_prompt`
- `analyze_append_prompt`
- `resolve_append_prompt`

When a field is present and non-empty, Conflux appends its value to the generated prompt for the corresponding operation before expanding `{prompt}` into the configured command. Existing prompt content is preserved and the append text is additive only.

The fields are deliberately top-level to avoid deep config nesting and mirror existing operation command keys.

## Acceptance Criteria

1. `OrchestratorConfig` accepts all five optional `*_append_prompt` fields from JSONC config files.
2. Config precedence and merge behavior for the new fields follows the existing per-field config precedence rules.
3. Each append prompt applies only to its matching operation.
4. Missing or empty append prompt values produce no extra prompt content and preserve current behavior.
5. Append text is added after the existing Conflux prompt contract, not before it and not as a replacement.
6. `cflx init` templates include commented examples for the new fields.
7. Tests prove at least acceptance and apply prompt injection through real command/prompt construction paths, not only field deserialization.

## Explicit Completion Conditions

- The config type and merge logic include all five `*_append_prompt` fields.
- Prompt construction for apply, acceptance, archive, analyze, and resolve uses the corresponding append prompt when present.
- Unit or integration tests verify config loading, default no-op behavior, and operation-specific append behavior.
- Template tests verify commented examples are emitted by generated init templates.
- `cargo test` passes for the touched areas.

## Out of Scope

- Adding `*_prepend_prompt` fields.
- Replacing built-in Conflux prompts.
- Auto-detecting optional tools such as `ocr`.
- Treating optional review tools as acceptance gates.
- Adding new command lifecycle hooks.
