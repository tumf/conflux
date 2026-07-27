## ADDED Requirements

### Requirement: Structured verification roles protect implementation dependency graphs

The existing proposal `verifications` metadata MUST be the single structured truth source for verification phase, execution environment, and completion role. Each declaration MAY add an execution class of `repository-local | repository-automation | deployed-service | physical-device | external-approval | credentialed-external` and a completion role of `change-blocking | operational-observation`. The validator MUST NOT introduce or require a parallel completion-gate declaration.

A `change-blocking` verification MUST be `pre-integration` and `repository-local`. A `post-integration` verification and any verification whose execution class is `repository-automation`, `deployed-service`, `physical-device`, `external-approval`, or `credentialed-external` MUST be an `operational-observation` and MUST NOT block Conflux acceptance, archive, or merge. Invalid combinations MUST fail on the proposal that owns the declaration.

Legacy verification declarations without the new fields MUST remain valid during migration. Strict validation MUST emit actionable migration warnings based on the existing structured phase rather than prose. Natural-language task or proposal content MUST NOT create workflow-control classifications.

Every checkbox in an active implementation task section MUST reference a declared `change-blocking` verification ID. Missing or unknown references and references to `operational-observation` declarations MUST fail validation. Narrative sections and Future Work MUST remain outside this linkage requirement. This linkage prevents external or manual completion gates from being hidden only in task prose while preserving prose as non-authoritative context.

#### Scenario: Repository-local implementation evidence blocks completion

**Given**: an implementation proposal declares a `pre-integration`, `repository-local`, `change-blocking` verification
**When**: strict validation runs
**Then**: the declaration is accepted as evidence that may block acceptance and archive

#### Scenario: Post-integration outcome cannot block change completion

**Given**: a proposal declares a `post-integration` verification
**When**: it marks that verification `change-blocking`
**Then**: strict validation fails on the owning proposal
**And**: the diagnostic requires `operational-observation`

#### Scenario: Physical-device acceptance is operational observation

**Given**: a verification uses execution class `physical-device`
**When**: strict validation evaluates its completion role
**Then**: only `operational-observation` is accepted
**And**: absence or failure of the device outcome does not keep the Conflux change active

#### Scenario: Legacy declaration receives migration warning

**Given**: an existing verification declaration has the previously valid fields but omits execution class and completion role
**When**: strict validation runs during the compatibility period
**Then**: validation does not fail solely because the new fields are absent
**And**: it emits an actionable warning derived from structured phase metadata

#### Scenario: Credentialed repository automation is not misclassified by prose

**Given**: a tracked repository workflow uses credentials after integration
**And**: it is declared `post-integration`, `repository-automation`, and `operational-observation`
**When**: strict validation runs
**Then**: the declaration is accepted without contacting the external system

#### Scenario: Active task cannot hide manual completion gate

**Given**: an active implementation checkbox requires physical-device or manual acceptance
**And**: it omits a verification reference or references an `operational-observation`
**When**: strict validation runs
**Then**: validation fails on the owning proposal
**And**: the diagnostic directs the author to move the outcome to Future Work or a release-observation change

### Requirement: Dependency validation prevents release gates from blocking implementation

Hard proposal dependencies MUST represent repository outputs required for implementation or pre-integration verification. A repository-local implementation change MUST NOT use roadmap order, MVP/release phase boundaries, deployed-service checks, physical-device acceptance, credentials, or external approval as its hard dependency justification.

When strict validation evaluates an active dependency edge, it MUST use valid structured verification declarations from both the dependent and target. It MUST reject an edge from a repository-local implementation change to a target that declares a non-local `change-blocking` verification. The diagnostic MUST identify the dependent change, dependency target, verification ID, and the corrective action to split release observation or remove the hard dependency. Correctly modeled operational-observation changes MAY depend on earlier operational-observation or implementation changes because those observations do not block Conflux completion.

Validation MUST remain offline and repository-local. Scheduler dependency resolution MUST remain unchanged.

#### Scenario: Local implementation dependency remains valid

**Given**: change `feature-b` depends on active change `feature-a`
**And**: `feature-a` declares only repository-local change-blocking evidence
**When**: strict validation evaluates `feature-b`
**Then**: the dependency edge is accepted

#### Scenario: Non-local blocker cannot hold implementation fan-out

**Given**: an active target has a non-local verification incorrectly declared change-blocking
**And**: fifteen repository-local implementation changes depend on that target
**When**: strict validation evaluates the graph
**Then**: the target declaration fails once with an owning-proposal diagnostic
**And**: each affected queued dependent receives a reference diagnostic naming the target and remedy
**And**: the graph is prevented from entering the same release-gate bottleneck

#### Scenario: Observational release chain remains valid

**Given**: release observation stage two depends on release observation stage one
**And**: both stages declare only operational-observation post-integration verification
**When**: strict validation evaluates stage two
**Then**: the dependency edge is not rejected by the release-gate rule

#### Scenario: Archived dependency behavior remains unchanged

**Given**: a dependency target is archived and integrated into the effective base
**When**: scheduler dependency resolution runs
**Then**: the dependency is resolved using existing archive and base-integration evidence
**And**: verification-role metadata does not alter scheduler semantics

### Requirement: Verification metadata changes preserve active workflow progress

Adding or changing structured verification metadata on an active target MUST report reverse-dependency impact before the new classification controls future dispatch. Malformed metadata errors MUST be owned by the target proposal; dependent diagnostics MUST reference that error rather than duplicate ambiguous parser findings.

A newly invalid edge MUST prevent not-yet-started or queued work from becoming dispatchable at the next eligibility decision. It MUST NOT abort an already in-flight apply, acceptance, archive, merge, or resolve operation solely because the target metadata changed after that operation started. This rule changes validation and dispatch eligibility only; it MUST NOT infer acceptance PASS or bypass archive checks.

#### Scenario: Target edit lists affected queued dependents

**Given**: an active target already has queued dependents
**When**: the target adds a structured non-local change blocker
**Then**: validation reports the affected dependent IDs
**And**: those queued dependents are ineligible at their next dispatch decision

#### Scenario: In-flight operation is not interrupted

**Given**: a dependent operation is already in flight
**When**: its active dependency target gains verification metadata that would reject a new edge
**Then**: the current operation is not aborted solely by that metadata edit
**And**: any later dispatch or retry evaluates the updated repository metadata

#### Scenario: Malformed target metadata has one owner

**Given**: an active target has malformed verification-role metadata and multiple dependents
**When**: strict validation evaluates the repository
**Then**: the field-specific primary error is attributed to the target
**And**: dependent diagnostics reference the invalid target without reproducing conflicting classifications

### Requirement: Proposal guidance separates implementation readiness from release observation

The bundled `cflx-proposal` skill MUST define hard dependencies as repository-output requirements for implementation or pre-integration verification. It MUST prohibit using hard dependencies solely for roadmap ordering, MVP/release phase boundaries, deployed-service checks, physical-device acceptance, credentials, or external approval. It MUST require authors to inspect direct and transitive downstream impact and split independently verifiable repository-local implementation from non-local release observation.

#### Scenario: Mixed local and release scope is split

**Given**: a requested change contains repository-local implementation and independently executable physical-device or deployed-service observation
**When**: `cflx-proposal` prepares the change structure
**Then**: guidance directs the author to create a locally completable implementation change and a separate release-observation change
**And**: the release-observation change may depend on the implementation change
**And**: unrelated follow-on implementation changes depend only on repository outputs they consume

#### Scenario: Dependency justification is implementation-specific

**Given**: a proposal author considers adding a dependency edge
**When**: bundled guidance evaluates that edge
**Then**: it requires identifying the concrete base-integrated code, contract, migration, or test surface consumed by the dependent change
**And**: release sequence alone is not accepted as justification
