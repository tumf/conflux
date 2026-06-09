---
change_type: implementation
priority: medium
dependencies: []
references:
  - "openspec/specs/configuration/spec.md"
  - "src/config/types.rs"
  - "src/agent/prompt.rs"
  - "src/execution/apply.rs"
  - "src/parallel/executor.rs"
  - "src/server/api/git_sync/resolve_command.rs"
---

# Change: add optional append prompts

**Change Type**: implementation

## Problem

Conflux builds operation prompts for agent commands such as `apply_command`, `acceptance_command`, `archive_command`, `analyze_command`, and `resolve_command`. These prompts encode Conflux workflow contracts, but users currently have no generic configuration surface for appending environment-specific guidance.

A concrete example is advising acceptance agents to optionally use local review tools such as `ocr review` as advisory evidence only. Such tools may be unavailable or misconfigured in a user's environment, and their findings can be wrong when they lack OpenSpec context, so Conflux should not hard-code them into commands or treat them as gates.

Existing `apply_prompt`, `acceptance_prompt`, and `archive_prompt` remain the operation's user-configurable base prompt inputs. This change adds a separate additive tail that is appended after the full generated operation prompt so users can add environment-specific guidance without replacing Conflux's built-in contract.

## Proposed Solution

Add top-level, optional operation-specific append prompt fields to `OrchestratorConfig`:

- `apply_append_prompt`
- `acceptance_append_prompt`
- `archive_append_prompt`
- `analyze_append_prompt`
- `resolve_append_prompt`

When a field is present and non-blank, Conflux appends its raw value to the final generated prompt for the corresponding operation before expanding `{prompt}` into the configured command. Existing prompt content is preserved and the append text is additive only.

The fields are deliberately top-level to avoid deep config nesting and mirror existing operation command keys.

The first implementation does not expand placeholders inside append prompt values. This keeps the feature narrow and avoids introducing operation-specific placeholder semantics before all prompt construction paths are aligned.

## Acceptance Criteria

1. `OrchestratorConfig` accepts all five optional `*_append_prompt` fields from JSONC config files.
2. Config precedence and merge behavior for the new fields follows the existing per-field config precedence rules.
3. Each append prompt applies only to its matching operation.
4. Missing, empty, or whitespace-only append prompt values produce no extra prompt content and preserve current behavior.
5. Append text is added after the final generated Conflux prompt for that operation, not before it and not as a replacement.
6. Append prompt values are treated as raw guidance text; placeholders inside them are not expanded.
7. Append prompt injection changes only the `{prompt}` value passed to operation command templates and does not change verdict parsing, lifecycle transitions, command availability checks, optional tool detection, or hook behavior.
8. `cflx init` templates include commented examples for the new fields, disabled by default.
9. Tests prove at least acceptance and apply prompt injection through real command/prompt construction paths, not only field deserialization.

## Explicit Completion Conditions

- The config type and merge logic include all five `*_append_prompt` fields.
- Prompt construction for apply, acceptance, archive, analyze, and resolve appends the corresponding non-blank raw append prompt at the final prompt tail.
- Tests verify config loading, merge precedence, default no-op behavior, whitespace-only no-op behavior, operation-specific append behavior, and absence of placeholder expansion inside append text.
- Template tests verify commented examples are emitted by generated init templates and remain inactive.
- `cflx openspec validate add-optional-append-prompt --strict --evidence warn` and the relevant Rust tests pass.

## Out of Scope

- Adding `*_prepend_prompt` fields.
- Replacing built-in Conflux prompts.
- Expanding `{change_id}` or any other placeholder inside append prompt values.
- Auto-detecting optional tools such as `ocr`.
- Treating optional review tools as acceptance gates.
- Changing acceptance verdict parsing, lifecycle state transitions, or hook semantics.
- Adding new command lifecycle hooks.
