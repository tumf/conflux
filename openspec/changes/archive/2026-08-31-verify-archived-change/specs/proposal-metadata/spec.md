## MODIFIED Requirements

### Requirement: proposal.md frontmatter metadata

`openspec/changes/<change-id>/proposal.md` MAY contain YAML frontmatter, and a proposal without frontmatter MUST remain readable. When frontmatter contains `verifications`, proposal tooling MUST parse it as an ordered list of structured verification declarations and MUST preserve the declarations when the proposal is read or archived. Runtime-supervised verification MUST resolve the declaration source from the active proposal when one exists, and otherwise from the sole canonical archive entry for the same logical change, using the repository's archive identity rules and failing closed when more than one canonical archive entry matches. Explicit verification metadata MUST remain authoritative over natural-language phase hints.

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

#### Scenario: archived change resolves the same verification declaration

**Given**: the active proposal is absent
**And**: exactly one canonical direct or dated archive entry contains the change proposal
**When**: runtime-supervised verification resolves the change by ID
**Then**: it reads the archived declaration with the same verification ID and repository-relative fields
**And**: it executes repository-local automation from the unchanged workspace root rather than from the archived directory

#### Scenario: an active proposal outranks a same-named archive entry

**Given**: an active proposal exists for the change ID
**And**: a canonical archive entry for the same change ID also exists
**When**: runtime-supervised verification resolves the change
**Then**: the active proposal is the declaration source
**And**: resolution does not fail on the coexistence

#### Scenario: multiple canonical archive entries fail closed

**Given**: the active proposal is absent
**And**: more than one canonical archive entry for the change ID contains `proposal.md`
**When**: runtime-supervised verification resolves the change
**Then**: resolution fails with an actionable ambiguity diagnostic naming the competing entries
**And**: no declaration or command is selected by filesystem iteration order

#### Scenario: invalid archive layout does not satisfy verification resolution

**Given**: only a nested date layout, malformed date, suffix collision, unrelated entry, or archive directory without `proposal.md` exists
**When**: runtime-supervised verification resolves the change
**Then**: the invalid entry is not accepted as the change's declaration source
**And**: a nested date layout reports the repository's existing invalid-archive-layout diagnostic
**And**: no verification command runs
