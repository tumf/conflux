## MODIFIED Requirements

### Requirement: proposal.md frontmatter metadata

`openspec/changes/<change-id>/proposal.md` MAY contain YAML frontmatter, and a proposal without frontmatter MUST remain readable. When frontmatter contains `verifications`, proposal tooling MUST parse it as an ordered list of structured verification declarations and MUST preserve the declarations when the proposal is read or archived. Runtime-supervised verification MUST resolve the declaration source from either the active proposal or the sole canonical archive entry for the same logical change, using the repository's archive identity rules and failing closed on conflicting or ambiguous identities. Explicit verification metadata MUST remain authoritative over natural-language phase hints.

#### Scenario: proposal with verification metadata is accepted

**Given**: `proposal.md` contains valid frontmatter with pre-integration and post-integration verification declarations
**When**: proposal-aware tooling reads the proposal
**Then**: both declarations are retained with their original IDs, phases, owners, paths, evidence locations, rerun actions, and prerequisites

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
**And**: it executes repository-local automation from the repository root

#### Scenario: ambiguous proposal identity fails closed

**Given**: active and archived proposal identities coexist, or more than one canonical archive entry matches the change ID
**When**: runtime-supervised verification resolves the change
**Then**: resolution fails with an actionable ambiguity diagnostic
**And**: no declaration or command is selected by filesystem iteration order

#### Scenario: invalid archive layout does not satisfy verification resolution

**Given**: only a nested date layout, malformed date, suffix collision, unrelated entry, or archive directory without `proposal.md` exists
**When**: runtime-supervised verification resolves the change
**Then**: the invalid entry is not accepted as the change's declaration source
**And**: no verification command runs
