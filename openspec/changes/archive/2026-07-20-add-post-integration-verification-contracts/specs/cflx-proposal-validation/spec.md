## MODIFIED Requirements

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
