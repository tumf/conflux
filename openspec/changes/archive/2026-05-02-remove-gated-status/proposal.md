---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/orchestration/state.rs
  - src/acceptance.rs
  - src/tui/state.rs
  - src/web/state.rs
  - src/server/api/ws.rs
  - src/events.rs
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/cli/spec.md
  - openspec/specs/agent-prompts/spec.md
---

# Change: Remove gated display state and fold acceptance gates into stalled

**Change Type**: implementation

## Premise / Context

- The user wants the lifecycle state model to stay simple after introducing `stalled`.
- `gated` currently appears as an acceptance-gate observation and user-facing display status, while `stalled` already represents resumable holds.
- Current canonical specs distinguish dependency `blocked`, apply/rejecting `stalled`, and acceptance `gated`, but the user prefers eliminating that extra displayed state.
- `rejected` must remain a terminal state distinct from resumable holds.
- The Conflux constitution requires workflow state to remain derivable from workspace/git/base state and completion to be repository-verifiable.

## Requested Artifact

- implementation proposal to remove the user-facing `gated` lifecycle/display state
- preserve acceptance blocker verdict parsing compatibility while representing non-terminal acceptance holds as `stalled`
- keep `rejected` as the only terminal rejection state

## Problem / Context

`gated` is a narrowly scoped acceptance observation, but exposing it as a lifecycle/display state makes the model harder to understand. After `stalled` exists for resumable holds, `gated` creates an unnecessary third blocker-adjacent user state between `blocked`, `stalled`, and `rejected`.

The desired model is simpler:

- `blocked` means dependency wait.
- `stalled` means non-terminal, resumable intervention/review hold.
- `rejected` means terminal rejection confirmed.

Acceptance blockers and rejection-review holds should therefore be surfaced as `stalled` with reason metadata rather than as a separate `gated` display state.

## Proposed Solution

Remove `gated` from user-facing lifecycle/display taxonomy and fold acceptance-gate observations into `stalled`.

- Replace reducer/display use of `WaitState::AcceptanceGated` with `WaitState::Stalled` plus blocker metadata such as `acceptance-gated`.
- Remove `gated` from TUI/Web/API display status values, filters, fixtures, legends, and tests.
- Keep acceptance parser compatibility for canonical `gated` verdict input and legacy `blocked` verdict input, but route both to non-terminal stalled/rejecting behavior rather than a `gated` display state.
- Update prompt/spec language so `gated` remains only an acceptance verdict/protocol input during compatibility, not a lifecycle state shown to users.
- Preserve rejection flow semantics: only confirmed rejection transitions to terminal `rejected`; resumable or review-required outcomes remain `stalled` or active `rejecting`.

## Acceptance Criteria

- No user-facing display status, WebSocket status, TUI filter, or frontend taxonomy exposes `gated` as a lifecycle state.
- Acceptance blocker outcomes that previously displayed `gated` now display `stalled` and preserve blocker reason metadata indicating an acceptance/review hold.
- Dependency waits still display `blocked` and remain distinguishable from stalled acceptance/rejection holds.
- Terminal rejection still displays `rejected` and remains non-requeueable without explicit marker removal or recovery.
- Acceptance parser compatibility continues to accept `ACCEPTANCE: GATED`, `{"acceptance":"gated"}`, and legacy blocked inputs where currently supported.
- Specs and prompt contracts no longer require `gated` as operator-facing wording or lifecycle taxonomy.

## Explicit Completion Conditions

- `src/orchestration/state.rs` no longer returns `"gated"` from `display_status()` and treats acceptance-gated waits as `stalled` with metadata.
- TUI/Web/API status mappings no longer include `gated`; tests/fixtures expecting `gated` are updated to `stalled` where the state is non-terminal and resumable.
- Acceptance parsing tests still prove `gated` verdict input is accepted for compatibility/protocol purposes.
- Rejection-flow tests still prove confirmed rejection reaches terminal `rejected` and resumable review/block outcomes do not.
- `cflx openspec validate remove-gated-status --strict --evidence warn` passes.
- Relevant Rust tests for reducer, acceptance parser, TUI/Web status mapping, and rejection routing pass.

## Out of Scope

- Removing acceptance `gated` verdict input compatibility entirely.
- Renaming `rejected` terminal state.
- Changing dependency analysis or dependency `blocked` semantics.
- Introducing durable workflow state outside workspace/git/base state.
