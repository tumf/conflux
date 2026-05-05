---
change_type: implementation
priority: medium
dependencies:
  - add-configurable-operation-skills
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/testing/spec.md
  - skills/cflx-accept/SKILL.md
  - .opencode/commands/cflx-accept.md
  - src/embedded_skills.rs
---

# Change: Add SPECA-oriented acceptance skill

**Change Type**: implementation

## Premise / Context

- Conflux acceptance currently has a standard operation skill `cflx-accept` and a fixed command template `.opencode/commands/cflx-accept.md` that owns the verdict protocol.
- The requested behavior is to add a concrete `cflx-accept-with-speca` skill so acceptance can perform SPECA-style property/proof-attempt review.
- The related `add-configurable-operation-skills` proposal makes `accept_skill` configurable, which is the intended opt-in path for this new skill.
- The new skill must preserve the existing Conflux acceptance verdict contract and must not introduce a second SPECA-specific terminal format.
- The Conflux constitution requires repository-verifiable evidence and prohibits hidden durable workflow-control state.

## Requested Artifact

Implementation proposal for a bundled `cflx-accept-with-speca` skill.

## Problem

Standard Conflux acceptance checks implementation evidence against OpenSpec changes, tasks, and parser contracts. When a change has implicit invariants or cross-module behavioral expectations, a SPECA-style property/proof-attempt pass can surface stronger findings: properties derived from specs and changed files are explicitly challenged against the implementation rather than relying only on checklist matching.

Today there is no bundled acceptance skill that tells agents how to run this SPECA-style review while still obeying Conflux acceptance output rules.

## Proposed Solution

Add `skills/cflx-accept-with-speca/SKILL.md` as a bundled operation skill.

The skill should:

1. Identify the operation as Conflux acceptance review with an additional SPECA-style property lens.
2. Read and follow `.opencode/commands/cflx-accept.md` / `cflx-accept` boundaries as the authoritative verdict contract.
3. Derive candidate properties from OpenSpec deltas, task claims, changed implementation paths, and constitution constraints.
4. Attempt to falsify those properties against repository evidence using available local commands, tests, static inspection, or an installed SPECA runner when available.
5. Classify proof-attempt results as blocking, advisory, incomplete, or gated.
6. Map blocking property failures to the existing JSON `fail` verdict with actionable `findings`.
7. Fall back to structured property review when SPECA tooling is unavailable, without returning a different output protocol.

## Acceptance Criteria

1. The bundled skill inventory includes `cflx-accept-with-speca` alongside `cflx-accept`.
2. The skill text clearly states that `.opencode/commands/cflx-accept.md` remains the single source of truth for fixed acceptance checks and final verdict format.
3. The skill defines a SPECA-style review loop: derive properties, attempt falsification/proof against repository evidence, classify outcomes, map to Conflux verdict.
4. The skill instructs agents not to ask the user questions and not to depend on out-of-worktree durable state for workflow-control decisions.
5. The skill treats missing SPECA tooling as a fallback to structured property review, not as an automatic pass or a new protocol.
6. Drift-detection tests prove the skill does not duplicate forbidden fixed acceptance formatting rules and preserves command-template ownership.
7. Documentation or config template guidance shows `"accept_skill": "cflx-accept-with-speca"` as the opt-in example once configurable operation skills are available.

## Explicit Completion Conditions

- `skills/cflx-accept-with-speca/SKILL.md` exists with valid skill frontmatter and concise operation guidance.
- `src/embedded_skills.rs` embeds and exposes the new skill.
- Embedded skill tests assert the built-in list contains `cflx-accept-with-speca`.
- Drift tests verify the new skill references the standard acceptance contract and does not copy command-template-only fixed procedure phrases.
- Documentation/templates include the opt-in example if the config option from `add-configurable-operation-skills` is implemented in the same branch.
- Targeted verification passes: `cargo test embedded_skills`, plus any prompt/config tests touched by documentation or template updates.
- `cflx openspec validate add-speca-acceptance-skill --strict --evidence warn` passes.

## Out of Scope

- Implementing or vendoring the external SPECA runner.
- Adding a `cflx speca` CLI subcommand.
- Changing `acceptance_command`, acceptance parser behavior, or verdict JSON schema.
- Persisting SPECA traces in durable workflow-control state.
- Making SPECA proof attempts mandatory when the environment lacks tooling.
