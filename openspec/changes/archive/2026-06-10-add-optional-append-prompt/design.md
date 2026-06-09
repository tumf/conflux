# Design: optional operation append prompts

## Context

Conflux has multiple operation prompt construction paths. Apply, archive, and acceptance prompts are assembled through `src/agent/prompt.rs` and invoked from callers such as `src/execution/apply.rs` and `src/parallel/executor.rs`. Analyze and resolve have their own command execution paths and must be wired at their actual prompt assembly sites rather than assumed to pass through the same helper.

Existing operation prompt fields such as `apply_prompt`, `acceptance_prompt`, and `archive_prompt` remain base user prompts. The new append prompt fields are a separate tail-only customization layer.

## Semantics

For each operation, Conflux first builds the complete prompt it would have built before this change. That prompt includes any built-in Conflux contract, selected skill prelude, user prompt, history context, diff context, retry context, and other operation-specific context.

After the complete existing prompt is built, Conflux appends the matching `*_append_prompt` value as the final section when the configured value is non-blank.

Blank means:

- field unset
- empty string
- whitespace-only string after trimming

Blank values are no-ops.

## Placeholder policy

Append prompt values are raw guidance text. The first implementation does not expand `{change_id}`, `{prompt}`, or any other placeholder inside append prompt values.

This avoids creating operation-specific placeholder semantics before all operation prompt construction paths are normalized.

## Non-interference policy

Append prompts affect only the generated prompt value that is substituted into operation command templates. They do not:

- change command template parsing
- change verdict marker parsing
- change acceptance PASS/CONTINUE/FAIL semantics
- change lifecycle state transitions
- auto-detect or execute optional tools
- alter hooks or shell command behavior

## Implementation shape

A small shared helper should normalize append behavior, for example:

```rust
fn append_optional_prompt(base: String, append: Option<&str>) -> String
```

The helper should trim only for the blank/no-op decision. If non-blank, it should append the original configured text as a new final section, preserving the user's intended formatting.

Each operation caller must pass the matching append prompt at the operation's actual prompt construction site:

- apply: apply prompt construction path around `src/execution/apply.rs`
- acceptance: acceptance prompt construction path around `src/parallel/executor.rs`
- archive: archive prompt construction path around `src/parallel/executor.rs`
- analyze: existing dependency-analysis prompt construction path
- resolve: existing resolve prompt construction path, including server/git-sync resolution where applicable

## Example

Config:

```jsonc
{
  "acceptance_append_prompt": "If locally available, you MAY use `ocr review` as advisory evidence only. Verify findings against OpenSpec before treating them as blockers."
}
```

Expected effect:

- acceptance prompt still contains the normal Conflux acceptance contract
- optional tool guidance appears as the final prompt section
- `ocr` is not executed automatically by Conflux
- acceptance cannot fail solely because an optional tool reported an issue
