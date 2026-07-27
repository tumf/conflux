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
  - openspec/changes/archive/2026-07-20-add-post-integration-verification-contracts
verifications:
  - id: release-gate-dependency-validation
    requirement: Proposal authoring and native validation prevent non-local release acceptance gates from becoming implementation dependency bottlenecks
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Cargo.toml
    evidence: cargo test openspec_cmd --lib output plus repository tests covering dependency gate diagnostics, migration safety, and skill guidance assertions
    rerun: cargo test openspec_cmd --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---
# Prevent Release Gates from Blocking Implementation Dependency Graphs

**Change Type**: implementation

## Problem / Context

Conflux correctly treats proposal `dependencies` as hard implementation-order gates: a dependent change remains blocked until each dependency is archived and integrated. Proposal authors can nevertheless place real-service, physical-device, external-approval, deployment, or other post-integration release acceptance inside an implementation change's completion conditions and then make follow-on implementation changes depend on it. That modeling turns an operational release gate into a graph-wide development stop even when the follow-on code only needs repository-local contracts or implementation.

Conflux already has structured `verifications` with `pre-integration | post-integration` phases. Post-integration outcomes belong to repository automation and do not block Conflux acceptance or archive. The bundled `cflx-proposal` skill also recommends mock-first verification, Future Work for non-AI-executable work, and proposal splitting. It does not explicitly define dependency eligibility, require reverse-dependency impact review, or prevent authors from restating post-integration/manual outcomes as active completion checkboxes. Native strict validation checks dependency target existence and verification structure but does not diagnose hazardous dependency composition.

## Proposed Solution

Define a deterministic dependency-authoring contract in the bundled `cflx-proposal` skill: hard dependencies are allowed only when the dependency's base-integrated repository output is required to implement or pre-integration-test the dependent change. Roadmap order, release phase boundaries, human approval, physical-device acceptance, deployed-service checks, credentials, and other non-local acceptance must not be represented as implementation dependencies or pre-integration completion tasks.

Extend the existing `verifications` declarations rather than introducing a second completion-gate truth source. Add an execution class and completion role to each structured verification. Validation shall accept change-blocking verification only for repository-local pre-integration evidence; post-integration, physical-device, external-approval, credentialed-external, and deployed-service outcomes must be operational observations that cannot block acceptance/archive. Every active implementation checkbox must reference a declared change-blocking verification ID, so non-local work cannot be hidden only in task prose. Conflicting field combinations and operational-observation IDs used by active tasks fail on the owning proposal.

Dependency diagnostics shall use only valid structured declarations. A repository-local implementation change that depends on an active target with a declared non-local change-blocking verification is rejected with a split/remove-dependency remedy. Correctly modeled post-integration observational changes may depend on one another because they do not block Conflux completion. Missing new fields on legacy declarations remain compatible with migration warnings; arbitrary prose never becomes workflow-control authority.

When one scope contains both locally completable implementation and non-local release acceptance, proposal guidance shall split it into an implementation change and a release-observation change. The release-observation change may depend on the implementation change, but unrelated follow-on implementation changes shall depend only on concrete repository changes they consume. Existing stuck graphs are not rewritten automatically; diagnostics shall identify the manual split path.

## Acceptance Criteria

- `cflx-proposal` distinguishes implementation dependencies from release sequencing and requires dependency justification based on consumed repository output.
- Proposal guidance requires direct and transitive downstream impact review before adding a dependency or non-local verification to an active target.
- Proposal guidance moves independently executable manual/external outcomes out of active implementation tasks and splits repository-local implementation from release observation when needed.
- Existing `verifications` declarations support fixed execution-class and completion-role fields; no parallel completion-gate metadata is introduced.
- Validation rejects invalid combinations, including any post-integration or non-local verification marked as change-blocking, and rejects active implementation tasks that omit a change-blocking verification reference or reference an operational observation.
- Strict validation rejects a repository-local implementation dependency edge to an active target with a validly declared non-local change blocker and identifies the dependent, target, verification ID, and remedy.
- Correctly modeled observational release changes may form ordered dependency chains without false rejection.
- Adding or changing verification metadata on an active target reports affected queued dependents without aborting already in-flight work; enforcement begins at the next dispatch eligibility decision.
- Malformed verification metadata errors belong to the target proposal; dependent diagnostics reference that target error rather than duplicating ambiguous parse failures.
- Legacy declarations without the new fields retain compatibility and receive migration warnings rather than immediate hard failures.
- Validation remains offline and repository-local; natural-language task/proposal content is advisory only.
- Ordinary dependencies on locally completable implementation changes remain valid, and scheduler hard-dependency semantics remain unchanged.

## Explicit Completion Conditions

- `skills/cflx-proposal/SKILL.md` documents dependency eligibility, forbidden release-sequencing uses, reverse-impact review, verification field rules, and the implementation/release-observation split pattern.
- `VerificationDeclaration` parsing adds fixed execution-class and completion-role values while preserving old declarations and rejecting unknown values or inconsistent new-field combinations.
- Native validation reuses structured verification metadata to diagnose hazardous active dependency edges without adding a second truth source or accessing a network.
- Validation defines target-owned malformed metadata diagnostics, dependent references, migration warnings, and queued-versus-in-flight enforcement timing.
- Regression tests cover the original fan-out shape with 15 dependents, valid local dependencies, invalid deployed-service and physical-device change blockers, observational release chains, legacy declarations, malformed metadata ownership, reverse-impact diagnostics, and in-flight non-interruption.
- `cargo test openspec_cmd --lib`, `cargo test openspec --lib`, `cargo fmt --check`, and `cargo clippy --all-targets --all-features -- -D warnings` pass.

## Out of Scope

- Automatically rewriting existing third-party OpenSpec proposals or repairing already stuck graphs.
- Softening or bypassing scheduler semantics for valid hard dependencies.
- Inferring release gates from arbitrary natural-language prose.
- Running external services, device checks, or deployment verification during validation.
- Introducing a new release lifecycle or external workflow-control state.
