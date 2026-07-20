## MODIFIED Requirements

### Requirement: proposal.md frontmatter metadata

`openspec/changes/<change-id>/proposal.md` MAY contain YAML frontmatter, and a proposal without frontmatter MUST remain readable. When frontmatter contains `verifications`, proposal tooling MUST parse it as an ordered list of structured verification declarations and MUST preserve the declarations when the proposal is read or archived. Explicit verification metadata MUST remain authoritative over natural-language phase hints.

#### Scenario: proposal with verification metadata is accepted

**Given**: `proposal.md` contains valid frontmatter with pre-integration and post-integration verification declarations
**When**: proposal-aware tooling reads the proposal
**Then**: both declarations are retained with their original IDs, phases, owners, paths, evidence locations, rerun actions, and prerequisites
**And**: the proposal body remains available unchanged

#### Scenario: legacy proposal remains readable

**Given**: `proposal.md` has no frontmatter or no `verifications` field
**When**: tolerant proposal tooling reads it
**Then**: the proposal remains readable
**And**: no verification declaration is invented from prose

#### Scenario: archived proposal preserves declarations

**Given**: an active proposal contains valid verification declarations
**When**: the change is archived through the native archive command
**Then**: the archived `proposal.md` retains declarations equivalent to the active proposal

## ADDED Requirements

### Requirement: proposal verification declarations

Each verification declaration MUST contain a unique non-empty `id`, a non-empty `requirement`, a `phase`, an `owner`, a non-empty `trigger`, a safe repository-relative `automation` path, a non-empty `evidence` location, a non-empty `rerun` action, and a `prerequisites` string list. `phase` MUST be `pre-integration` or `post-integration`. A pre-integration declaration MUST use `owner: conflux-acceptance`; a post-integration declaration MUST use `owner: repository-automation`.

#### Scenario: pre-integration declaration identifies repository verification

**Given**: an implementation proposal declares `phase: pre-integration`
**When**: metadata is parsed
**Then**: the declaration identifies Conflux acceptance ownership and a tracked repository automation file

#### Scenario: post-integration declaration identifies operational ownership

**Given**: a proposal declares `phase: post-integration`
**When**: metadata is parsed
**Then**: the declaration identifies repository automation ownership, its trigger, evidence location, rerun action, and external prerequisites

#### Scenario: explicit phase wins over contradictory prose

**Given**: a structured declaration says `phase: post-integration`
**And**: proposal prose could be interpreted as pre-integration
**When**: tooling classifies the verification phase
**Then**: it uses `post-integration`
**And**: prose analysis may emit an advisory warning but cannot change routing semantics
