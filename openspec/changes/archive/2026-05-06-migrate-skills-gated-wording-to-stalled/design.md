# Design: Stalled-first skill terminology with gated protocol compatibility

## Current State

The current lifecycle taxonomy distinguishes:

- `blocked`: dependency wait before dispatch
- `stalled`: non-terminal resumable or review-required hold
- `rejected`: terminal rejection

Acceptance review still uses `{"acceptance":"gated"}` and `ACCEPTANCE: GATED` as parser-compatible handoff tokens for implementation blockers. That compatibility token is narrower than a lifecycle state: it means acceptance found a valid blocker that repository-only autonomous work cannot resolve.

## Target Wording Model

Distributed skills should teach the conceptual model first:

1. A valid Implementation Blocker creates a **stalled acceptance hold**.
2. The runtime/user-facing lifecycle is `stalled`.
3. Current runtime-compatible machine-readable handoff remains `{"acceptance":"gated"}` and, for older fallback handling, `ACCEPTANCE: GATED`.
4. `gated` is not a lifecycle/status name and not the primary rubric name.

This keeps installed skills aligned with the product model without breaking deployed runtimes that still parse `gated`.

## Compatibility Boundary

The proposal intentionally does not ask skills to emit `{"acceptance":"stalled"}` yet. Current parser behavior treats unrecognized acceptance kinds as non-terminal continuation, so issuing a stalled verdict before parser support could fail to preserve a valid implementation blocker hold.

A later runtime proposal can introduce a new verdict shape such as:

```json
{"acceptance":"stalled","reason":"implementation_blocker","findings":["<evidence>"]}
```

After that parser support is released, the distributed skills can switch the primary machine-readable output and demote `gated` further to legacy input/output compatibility.

## Affected Surfaces

- `skills/cflx-accept/SKILL.md`: authoritative portable acceptance skill.
- `skills/cflx-accept-with-speca/SKILL.md`: drop-in property-review variant sharing the same interface.
- `skills/cflx-workflow/SKILL.md` and `skills/cflx-workflow/references/cflx-accept.md`: legacy workflow guidance and references that still teach the acceptance contract.
- `skills/cflx-archive/SKILL.md`, `skills/README.md`, and rejection-guide text: secondary references that should not reintroduce lifecycle confusion.
- `.opencode/commands/cflx-accept.md`: adapter/mirror for OpenCode users, not authoritative but visible.
- `src/embedded_skills.rs`: embedded-skill tests should protect the installed bundle from drifting back to `GATED`-centered guidance.

## Verification Strategy

Use repository-verifiable text and contract tests rather than runtime behavior changes:

- strict OpenSpec validation for the proposal and spec delta
- targeted markdown search/review over distributed skills
- embedded skill tests that inspect bundled skill strings
- no parser or orchestration behavior changes in this proposal
