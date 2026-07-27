---
change_type: implementation
priority: high
dependencies: []
references:
  - skills/cflx-proposal/SKILL.md
  - src/openspec_cmd/validation.rs
  - src/openspec.rs
  - src/parallel/dependency.rs
  - openspec/specs/cflx-proposal-validation/spec.md
verifications:
  - id: release-gate-dependency-validation
    requirement: Proposal authoring and native validation prevent non-local release acceptance gates from becoming implementation dependency bottlenecks
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: cargo test openspec_cmd --lib output plus repository tests covering dependency gate diagnostics and skill guidance assertions
    rerun: cargo test openspec_cmd --lib
    prerequisites: []
---
# Prevent Release Gates from Blocking Implementation Dependency Graphs

**Change Type**: implementation

## Problem / Context

Conflux correctly treats proposal `dependencies` as hard implementation-order gates: a dependent change remains blocked until each dependency is archived and integrated. Proposal authors can nevertheless place real-service, physical-device, external-approval, deployment, or other post-integration release acceptance inside an implementation change's completion conditions and then make follow-on implementation changes depend on it. That modeling turns an operational release gate into a graph-wide development stop even when the follow-on code only needs repository-local contracts or implementation.

The bundled `cflx-proposal` skill already recommends mock-first verification, post-integration declarations, Future Work for non-AI-executable work, and proposal splitting. It does not explicitly define when `dependencies` are valid, require reverse-dependency impact review, or direct authors to separate implementation readiness from release acceptance. Native strict validation checks dependency target existence and structured verification declarations but does not diagnose this hazardous composition.

## Proposed Solution

Define a deterministic dependency-authoring contract in the bundled `cflx-proposal` skill: hard dependencies are allowed only when the dependency's base-integrated repository output is required to implement or pre-integration-test the dependent change. Roadmap order, release phase boundaries, human approval, physical-device acceptance, deployed-service checks, credentials, and other non-local acceptance must not be represented as implementation dependencies.

Extend native OpenSpec validation with a structured, repository-local gate classification that does not infer workflow authority from prose. Add optional machine-readable proposal metadata for completion gates, with each gate declaring its phase and execution class. Strict validation shall reject a dependency edge when the target change declares a completion-blocking gate that is post-integration or non-local. Missing gate metadata remains backward-compatible and advisory; validation must not guess from words such as “manual”, URLs, or task prose.

When one scope contains both locally completable implementation and non-local release acceptance, proposal guidance shall split it into an implementation change and a release-acceptance change. The release-acceptance change may depend on the implementation change, but unrelated follow-on implementation changes shall depend only on the concrete repository changes they consume.

## Acceptance Criteria

- `cflx-proposal` explicitly distinguishes implementation dependencies from release sequencing and requires dependency justification based on consumed repository output.
- Proposal guidance requires checking direct and transitive downstream impact before making a change with non-local completion gates a dependency.
- Proposal guidance requires split changes when repository-local implementation completion and non-local release acceptance can be verified independently.
- Proposal frontmatter can declare completion gates using deterministic fields for phase, execution class, and whether the gate blocks change completion.
- Strict validation rejects a dependency edge to an active target whose structured completion metadata includes a blocking `post-integration` or non-local gate.
- Validation diagnostics identify the dependent change, dependency target, offending gate ID, and the corrective split/remove-dependency action.
- Validation remains offline and repository-local; natural-language task/proposal content is advisory only.
- Existing proposals without completion-gate metadata retain compatibility and receive no hard failure solely because metadata is absent.
- Ordinary dependencies on locally completable implementation changes remain valid.

## Explicit Completion Conditions

- `skills/cflx-proposal/SKILL.md` documents the hard-dependency eligibility rule, forbidden release-sequencing uses, downstream impact review, and split pattern.
- Proposal metadata parsing accepts and validates completion-gate declarations with fixed enums and required non-empty fields while rejecting malformed or duplicate declarations.
- Native validation resolves active dependency proposal files and emits deterministic errors for structured blocking post-integration/non-local gates without network access.
- Unit tests cover a valid local implementation dependency, invalid deployed-service and physical-device blockers, malformed metadata, duplicate gate IDs, missing dependency metadata compatibility, and transitive graph impact diagnostics where applicable.
- `cargo test openspec_cmd --lib`, `cargo fmt --check`, and `cargo clippy --all-targets --all-features -- -D warnings` pass.

## Out of Scope

- Automatically rewriting existing third-party OpenSpec proposals.
- Changing scheduler semantics for valid hard dependencies.
- Inferring release gates from arbitrary natural-language prose.
- Running external services, device checks, or deployment verification during validation.
