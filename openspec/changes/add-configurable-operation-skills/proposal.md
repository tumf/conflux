---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/configuration/spec.md
  - openspec/specs/agent-prompts/spec.md
  - src/config/types.rs
  - src/agent/prompt.rs
  - src/orchestration/selection.rs
  - src/orchestration/rejection.rs
  - src/parallel/conflict.rs
  - src/embedded_skills.rs
---

# Change: Add configurable operation skills

**Change Type**: implementation

## Premise / Context

- 中断前に、既存の `add-configurable-accept-skill` proposal ディレクトリを `add-configurable-operation-skills` に rename 済み。
- 固定 `load skills:` は acceptance だけでなく、analyze / apply / rejecting / cleanup-review / archive / resolve にも埋め込まれている。
- 主な該当箇所は `src/agent/prompt.rs`, `src/orchestration/selection.rs`, `src/orchestration/rejection.rs`, `src/parallel/conflict.rs`。
- ユーザー意図は、`accept_skill` だけを特別扱いせず、他の orchestrator operation skill も config で差し替え可能にすること。
- 既存の command template / verdict parser / workflow-control state は変えず、可変 prompt の skill prelude だけを設定化する。

## Requested Artifact

Implementation proposal for configurable skill selection across Conflux orchestrator operations.

## Problem

Conflux currently loads operation-specific skills through hardcoded prompt preludes such as:

```text
load skills: cflx-apply
load skills: cflx-accept
load skills: cflx-resolve
```

This prevents projects from swapping in specialized operation skills while keeping the standard command templates, retry/history context, prompt structure, and parser contracts. Acceptance is the immediate SPECA use case, but the same limitation exists for apply, archive, conflict resolution, analysis, rejecting review, and cleanup-review.

## Proposed Solution

Add optional top-level config keys for orchestrator-loaded operation skills:

```jsonc
{
  "analyze_skill": "cflx-analyze",
  "apply_skill": "cflx-apply",
  "rejecting_skill": "cflx-rejecting",
  "cleanup_review_skill": "cflx-cleanup-review",
  "accept_skill": "cflx-accept",
  "archive_skill": "cflx-archive",
  "resolve_skill": "cflx-resolve"
}
```

Behavior:

1. If a key is omitted, Conflux uses the current built-in default skill name for that operation.
2. If a key is set, prompt construction emits `load skills: <configured-name>` for that operation.
3. Config merge precedence follows existing top-level config rules.
4. Skill selection affects only prompt guidance. It does not change command execution, parser behavior, archive routing, dependency selection semantics, or workflow-control state by itself.
5. Existing prompt context order and fixed-procedure ownership remain unchanged.

## Acceptance Criteria

1. Omitting all new skill config keys preserves the current skill preludes for analyze, apply, rejecting, cleanup-review, accept, archive, and resolve.
2. Setting a custom operation skill changes only that operation's `load skills:` prelude.
3. `accept_skill = "cflx-accept-with-speca"` causes acceptance prompt construction to load `cflx-accept-with-speca` without replacing `acceptance_command`.
4. Config precedence works for all operation skill keys.
5. Prompt builders still include their existing variable context in the same relative order after the selected skill prelude.
6. Fixed procedures remain owned by their existing command templates / skill documents; Rust-side prompt builders do not grow duplicated checklists.
7. Existing parsers and terminal verdict markers remain unchanged.

## Explicit Completion Conditions

- `OrchestratorConfig` includes optional storage, merge behavior, accessors/defaults, and tests for all operation skill keys.
- Prompt builders accept or obtain the configured skill names instead of hardcoding `cflx-*` operation skill names.
- Call sites pass the effective skill names from config to analyze/apply/rejecting/cleanup-review/accept/archive/resolve prompt construction.
- `src/templates.rs` or config documentation shows the default skill names and at least one custom example.
- Targeted tests cover default preservation, per-operation override, config precedence, and unchanged parser behavior.
- `cflx openspec validate add-configurable-operation-skills --strict --evidence warn` passes.

## Out of Scope

- Creating the concrete `cflx-accept-with-speca` skill content; that is handled by the dependent `add-speca-acceptance-skill` proposal.
- Changing `acceptance_command`, `apply_command`, `archive_command`, `resolve_command`, or `analyze_command` semantics.
- Adding prompt-file / stdin handoff.
- Validating at config-load time that an external agent runtime can actually load the configured skill.
- Making human-triggered OpenCode commands such as `/cflx-proposal` configurable.
