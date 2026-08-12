---
change_type: spec-only
priority: high
dependencies: []
references:
  - openspec/changes/archive/2026-08-12-add-client-cli
  - openspec/changes/archive/2026-08-12-fix-client-cli-contract
  - openspec/specs/cli/spec.md
  - openspec/specs/remote-control-api/spec.md
---

# Preserve complete client CLI canonical requirements

**Change Type**: spec-only

## Premise / Context

- `fix-client-cli-contract` implemented and merged the four reviewed corrections.
- Its archive promotion replaced complete `MODIFIED` requirements with only the scenarios repeated in the corrective delta.
- Existing scenarios from `add-client-cli` disappeared even though runtime behavior was not removed.
- This repairs canonical specs only; product code and runtime behavior must not change.

## Requested Artifact

Specification repair combining the original complete scenario sets with the corrective descriptions and scenarios.

## Problem / Context

Canonical requirements now omit previously accepted scenarios for owner isolation, enqueue safety, coherent observation, completion proof, API capability discovery, and read-only behavior. This falsely weakens the contract and future acceptance.

## Proposed Solution

Restore every prior scenario under the four modified requirements while retaining all corrections added by `fix-client-cli-contract`.

## Acceptance Criteria

1. The three client requirements contain every scenario from `add-client-cli` plus the corrective scenarios.
2. `Local client compatibility discovery` contains every prior scenario plus bearer-token scenarios.
3. No product source or test file changes.
4. Strict and archive-gate validation pass.

## Explicit Completion Conditions

- Canonical promotion changes only the intended two spec files.
- No scenario heading from either archived change is absent from the resulting requirement.
- Strict evidence validation and archive gate pass.

## Out of Scope

- Runtime implementation changes.
- New client behavior or API fields.
- Rewriting archived changes.
