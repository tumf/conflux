## Requirements

### Requirement: evidence-hint-matching

The OpenSpec archive gate MUST evaluate repository-verifiable evidence and ownership markers from the complete task verification note, not from a truncated substring caused by parenthesized or backticked command/prose content inside the note. The evidence matcher MUST accept generic repository-evidence vocabulary used by Conflux diagnostics and proposal guidance when it appears with a valid verification ownership marker, including source-path, test-file, and runnable-command wording. The matcher MUST also accept common concrete repository artifact and build-command evidence such as Dockerfiles, TOML configuration files, and Docker build commands. Weak narrative notes without concrete repository evidence MUST remain rejected.

#### Scenario: Manual verification note contains source paths and runnable command

**Given**: an implementation change task line contains an inline `(verification: manual - ...)` note with source paths and a runnable `cflx openspec validate <id> --strict` command
**When**: `cflx openspec validate <id> --archive-gate` evaluates the task
**Then**: the archive gate accepts the task's verification evidence instead of reporting that repository-verifiable evidence is missing

#### Scenario: Verification note contains generic evidence vocabulary

**Given**: an implementation change task line contains a verification note with a valid ownership marker and generic repository evidence wording such as `source paths`, `test files`, or `runnable command`
**When**: `cflx openspec validate <id> --archive-gate` evaluates the task
**Then**: the archive gate accepts the note as repository-verifiable evidence

#### Scenario: Verification note contains common build artifacts and commands

**Given**: an implementation change task line contains a verification note with a valid ownership marker and evidence such as `Dockerfile`, `.toml`, or `docker build`
**When**: `cflx openspec validate <id> --archive-gate` evaluates the task
**Then**: the archive gate accepts the note as repository-verifiable evidence

#### Scenario: Verification note contains parenthesized command or prose content

**Given**: an inline verification note contains parenthesized or backticked command/prose segments before the repository evidence hint
**When**: the validator extracts the verification note
**Then**: extraction includes the full evidence-bearing note rather than stopping at the first inner closing parenthesis

#### Scenario: Missing or weak verification remains rejected

**Given**: an implementation change task has no verification note, lacks a recognized verification ownership marker, or lacks repository-verifiable evidence
**When**: the archive gate evaluates the task
**Then**: the archive gate continues to emit the appropriate strict validation finding

### Requirement: no-delta-marker-validation

Strict validation MUST accept a change that has a `specs/.no-delta` marker file and no spec delta directories. The `.no-delta` file declares that the change intentionally carries no spec modifications.

#### Scenario: Change with .no-delta marker passes strict validation

**Given**: A change directory contains `specs/.no-delta` and no subdirectories under `specs/`
**When**: `cflx openspec validate <id> --strict` is executed
**Then**: Validation passes without spec delta errors

#### Scenario: .no-delta marker conflicts with existing spec deltas

**Given**: A change directory contains both `specs/.no-delta` and one or more spec delta subdirectories under `specs/`
**When**: `cflx openspec validate <id> --strict` is executed
**Then**: Validation fails with an error indicating `.no-delta` conflicts with existing spec deltas

#### Scenario: No .no-delta and no spec deltas fails strict validation

**Given**: A change directory has no `specs/.no-delta` file and no spec delta subdirectories under `specs/`
**When**: `cflx openspec validate <id> --strict` is executed
**Then**: Validation fails with an error indicating no spec deltas found (unchanged from current behavior)

## Requirements

### Requirement: change-directory-validity-filter

`cflx openspec` の change 列挙および change 解決処理は、`proposal.md` が存在しないディレクトリを有効な change として扱ってはならない（MUST NOT）。invalid ディレクトリを検出した場合は stderr に警告を出力しなければならない（MUST）。

#### Scenario: proposal.md のないディレクトリが list から除外される

- **GIVEN** `openspec/changes/broken-dir/` が存在するが `proposal.md` を含まない
- **WHEN** `cflx openspec list` を実行する
- **THEN** `broken-dir` は change 一覧に表示されない
- **AND** stderr に `broken-dir` に関する警告が出力される

#### Scenario: proposal.md のあるディレクトリは従来どおり表示される

- **GIVEN** `openspec/changes/valid-change/proposal.md` が存在する
- **WHEN** `cflx openspec list` を実行する
- **THEN** `valid-change` は change 一覧に表示される

#### Scenario: _find_change_dir が invalid ディレクトリを返さない

- **GIVEN** `openspec/changes/ghost-dir/` が存在するが `proposal.md` を含まない
- **WHEN** `cflx openspec show ghost-dir` を実行する
- **THEN** change が見つからないエラーが返る

## Requirements

### Requirement: Bundled skills use native OpenSpec CLI commands

Conflux skill sources, active canonical specs, and repository-facing validator guidance MUST instruct agents and users to call native `cflx openspec` subcommands for list/show/validate/archive operations. The repository MUST NOT depend on `skills/cflx-proposal/scripts/cflx.py` as an executable validator contract once native CLI parity is complete.

#### Scenario: repository no longer retains cflx.py validator helper

- **GIVEN** the native Rust validator already covers the proposal validation contract used by active Conflux workflows
- **WHEN** repository validation and skill-distribution checks are executed
- **THEN** the repository does not contain `skills/cflx-proposal/scripts/cflx.py`
- **AND** proposal validation continues to work through `cflx openspec validate ...`
- **AND** no active canonical spec or skill-facing documentation instructs the user to execute `cflx.py`

### Requirement: Native validator owns behavior-centric proposal checks

The native `cflx openspec validate` implementation MUST enforce deterministic proposal authoring contracts, including structured verification declarations, without using natural-language inference as workflow-control authority. In strict and archive-gate modes, malformed declarations, missing required fields, duplicate IDs, invalid phase/owner relationships, empty required values, unsafe automation paths, and automation paths that do not identify an existing tracked repository regular file MUST fail validation with actionable diagnostics.

Implementation and hybrid proposals MUST declare at least one pre-integration verification. Spec-only proposals MAY omit verification declarations. A post-integration declaration MUST identify repository automation ownership, trigger, evidence location, rerun action, and prerequisites. Validation MUST NOT access a network, external API, or deployed target to validate any declaration.

#### Scenario: strict validation accepts a complete post-integration contract

**Given**: an implementation proposal has a pre-integration verification
**And**: it declares a post-integration verification whose automation path is a tracked repository workflow file
**And**: all required fields and phase/owner relationships are valid
**When**: `cflx openspec validate alpha --strict` runs
**Then**: verification declaration validation succeeds without accessing the external target

#### Scenario: strict validation rejects ownerless cyclic gate

**Given**: an implementation proposal requires an outcome available only after integration
**And**: its post-integration declaration omits repository automation ownership or a rerun action
**When**: strict validation runs
**Then**: validation fails before apply
**And**: the diagnostic identifies the missing declaration field

#### Scenario: unsafe automation path is rejected

**Given**: a verification declaration uses an absolute path, parent traversal, external symlink, missing file, or non-regular file as `automation`
**When**: strict or archive-gate validation runs
**Then**: validation fails with the offending verification ID and path

#### Scenario: natural-language phase inference is advisory only

**Given**: proposal prose appears to describe a different verification phase from its structured declaration
**When**: validation runs
**Then**: the structured phase remains authoritative
**And**: any prose-based diagnostic is advisory and cannot create or change workflow routing

#### Scenario: implementation proposal requires repository-verifiable pre-integration evidence

**Given**: an implementation or hybrid proposal has no pre-integration verification declaration
**When**: strict validation runs
**Then**: validation fails with guidance to declare repository-verifiable implementation evidence
**And**: a spec-only proposal without declarations remains valid

### Requirement: self-referential-final-validation-task-guard

The native `cflx openspec validate` implementation SHALL reject checkbox tasks that require final OpenSpec validation of the same change as implementation evidence. Final OpenSpec validation is an archive gate, not an implementation checkbox task.

The validator SHALL allow non-checkbox final validation guidance sections to mention the same validation command. Runtime-owned `## Acceptance #N Failure Follow-up` sections SHALL NOT be treated as implementation task declarations for self-referential final-validation or repository-evidence validation because they mirror acceptance findings rather than proposal-authored implementation tasks.

#### Scenario: same-change final validation checkbox fails

**Given**: an active change `alpha`
**And**: `openspec/changes/alpha/tasks.md` contains a checkbox task that asks the implementer to run or record `cflx openspec validate alpha`
**When**: `cflx openspec validate alpha --strict --evidence error` is executed
**Then**: validation fails
**And**: the diagnostic identifies the task as a self-referential final validation checkbox
**And**: the diagnostic tells the author to move final validation to a non-checkbox `Final Validation` section

#### Scenario: non-checkbox final validation section passes

**Given**: an active change `alpha`
**And**: `openspec/changes/alpha/tasks.md` contains a non-checkbox `## Final Validation` section mentioning `cflx openspec validate alpha --strict --evidence warn`
**And**: all implementation checkbox tasks have valid repository-verifiable evidence notes
**When**: `cflx openspec validate alpha --strict --evidence error` is executed
**Then**: validation passes without a self-referential final validation diagnostic

#### Scenario: runtime-owned acceptance follow-up is not revalidated as implementation tasks

**Given**: an active change `alpha`
**And**: `openspec/changes/alpha/tasks.md` contains a runtime-owned `## Acceptance #2 Failure Follow-up` section
**And**: that section mirrors an archive-gate or repository-fixable acceptance finding as a checkbox without a proposal verification declaration
**When**: `cflx openspec validate alpha --strict --evidence error` is executed
**Then**: validation does not emit self-referential final-validation or missing-verification diagnostics for that runtime-owned checkbox
**And**: ordinary implementation tasks outside the runtime-owned section remain subject to those diagnostics

#### Scenario: ordinary repository evidence remains accepted

**Given**: an active change `alpha`
**And**: `openspec/changes/alpha/tasks.md` contains implementation checkbox tasks verified by runnable commands such as `cargo test`, `npm run`, `go test`, or source/test file paths
**When**: `cflx openspec validate alpha --strict --evidence error` is executed
**Then**: those ordinary verification notes remain accepted

### Requirement: archive-equivalent-validation-command

Conflux SHALL provide a documented local validation command whose failure policy matches archive readiness. Users and agents MUST be able to reproduce archive-blocking evidence findings before invoking archive.

Validation errors and an invalid validation result MUST block archive. Advisory warnings, including archived dependency reference warnings, MUST NOT block archive when no validation error exists. The archive-equivalent validation command and actual archive operation MUST apply the same blocking distinction between errors and warnings.

#### Scenario: local archive gate fails on evidence findings

**Given**: an active change `alpha` has an evidence finding that would block archive
**When**: the documented archive-equivalent validation command is executed locally
**Then**: the command exits non-zero
**And**: it reports the same evidence finding that would block archive

#### Scenario: archive failure message includes local reproduction command

**Given**: archive fails because validation found evidence issues
**When**: Conflux reports the archive failure
**Then**: the message includes the local archive-equivalent validation command for the same change
**And**: the message preserves the specific validation finding instead of reporting only generic archive verification failure

### Requirement: Task section classification is consistent

The native OpenSpec validator MUST classify each top-level `tasks.md` section as an active task section, a narrative non-task section, or a runtime-owned acceptance follow-up section. Task counting and task validation MUST use the same classification contract.

Final Validation, Implementation Blocker, Future Work, Out of Scope, Notes, and Acceptance Notes MUST be narrative non-task sections. Runtime-owned current and numbered acceptance failure follow-up sections MUST retain their dedicated runtime classification.

#### Scenario: Active section bare bullet remains invalid

**Given**: an active implementation section contains a top-level `- evidence: command passed` or another non-checkbox task-like bullet
**When**: strict or archive-gate validation runs
**Then**: validation fails with an actionable `Possible task without checkbox` diagnostic

#### Scenario: Narrative section permits ordinary bullets

**Given**: Final Validation or Implementation Blocker contains ordinary prose or non-checkbox metadata bullets
**When**: strict or archive-gate validation runs
**Then**: those narrative bullets are not reported as unchecked tasks
**And**: they are not included in task progress totals

#### Scenario: Narrative section rejects checkboxes

**Given**: a narrative non-task section contains `- [ ]` or `- [x]`
**When**: strict or archive-gate validation runs
**Then**: validation fails with a diagnostic requiring removal of the checkbox

#### Scenario: Section transition restores active validation

**Given**: a narrative section with permitted bullets is followed by an active implementation section with a bare task bullet
**When**: validation runs
**Then**: only the bare bullet in the active section is rejected

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
