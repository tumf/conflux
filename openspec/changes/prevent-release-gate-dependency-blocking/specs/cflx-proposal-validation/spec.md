## ADDED Requirements

### Requirement: Structured completion gates protect implementation dependency graphs

Proposal metadata MAY declare completion gates using stable gate IDs, a phase of `pre-integration | post-integration`, an execution class of `repository-local | deployed-service | physical-device | external-approval | credentialed-external`, and an explicit boolean indicating whether the gate blocks change completion. Strict validation MUST validate these declarations deterministically and MUST NOT infer gate authority from proposal or task prose.

When an active proposal declares a dependency on another active change, strict validation MUST reject the edge if the dependency target declares a completion-blocking gate whose phase is `post-integration` or whose execution class is not `repository-local`. The diagnostic MUST identify the dependent change, dependency target, gate ID, and the corrective action to split release acceptance or remove the hard dependency. Validation MUST remain offline and repository-local.

Proposals without completion-gate metadata MUST remain backward-compatible and MUST NOT fail solely because metadata is absent. Dependencies on targets whose blocking gates are repository-local and pre-integration MUST remain valid.

#### Scenario: Local implementation dependency remains valid

**Given**: change `feature-b` depends on active change `feature-a`
**And**: `feature-a` declares only completion-blocking `pre-integration` and `repository-local` gates
**When**: strict validation evaluates `feature-b`
**Then**: the dependency edge is accepted

#### Scenario: Deployed release acceptance cannot block implementation

**Given**: change `feature-b` depends on active change `release-a`
**And**: `release-a` declares a completion-blocking gate with phase `post-integration` and execution class `deployed-service`
**When**: strict validation evaluates `feature-b`
**Then**: validation fails without contacting the deployed service
**And**: the diagnostic names `feature-b`, `release-a`, and the offending gate ID
**And**: the diagnostic directs the author to split release acceptance or remove the hard dependency

#### Scenario: Physical-device acceptance cannot become a hard dependency

**Given**: an active dependency target declares a completion-blocking `physical-device` gate
**When**: another implementation change declares that target as a dependency
**Then**: strict validation rejects the dependency edge

#### Scenario: Missing metadata remains compatible

**Given**: an active dependency target predates completion-gate metadata
**When**: strict validation evaluates a dependent proposal
**Then**: validation does not fail solely because the target has no completion-gate declarations
**And**: no prose-derived gate classification controls validation

#### Scenario: Malformed gate declaration fails deterministically

**Given**: a proposal declares an empty gate ID, duplicate gate IDs, an unsupported phase or execution class, or omits the blocking boolean
**When**: strict validation runs
**Then**: validation fails with a field-specific diagnostic

### Requirement: Proposal guidance separates implementation readiness from release acceptance

The bundled `cflx-proposal` skill MUST define hard dependencies as repository-output requirements for implementation or pre-integration verification. It MUST prohibit using hard dependencies solely for roadmap ordering, MVP/release phase boundaries, deployed-service checks, physical-device acceptance, credentials, or external approval. It MUST require authors to inspect downstream impact and split independently verifiable repository-local implementation from non-local release acceptance.

#### Scenario: Mixed local and release scope is split

**Given**: a requested change contains repository-local implementation and independently executable physical-device or deployed-service release acceptance
**When**: `cflx-proposal` prepares the change structure
**Then**: guidance directs the author to create a locally completable implementation change and a separate release-acceptance change
**And**: the release-acceptance change may depend on the implementation change
**And**: unrelated follow-on implementation changes depend only on repository outputs they consume

#### Scenario: Dependency justification is implementation-specific

**Given**: a proposal author considers adding a dependency edge
**When**: the bundled guidance evaluates that edge
**Then**: it requires identifying the concrete base-integrated code, contract, migration, or test surface consumed by the dependent change
**And**: release sequence alone is not accepted as justification
