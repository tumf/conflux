## MODIFIED Requirements

### Requirement: Dependency target classification uses repository-visible evidence

Dependency target classification SHALL collect archived, rejected, queued, and in-flight dependency evidence from the target repository being orchestrated. Archived dependency targets SHALL be treated as already satisfied references and MUST NOT be misclassified as missing because the Conflux process current working directory differs from the target repository root.

#### Scenario: Archived dependency classification is independent of process cwd

**Given**: the target repository contains `openspec/changes/archive/2026-05-10-base/proposal.md`
**And**: an active queued change `dependent` declares dependency `base`
**And**: Conflux is launched or the analyzer is instantiated while the process cwd is outside the target repository
**When**: dependency analysis validates `dependent`
**Then**: dependency `base` is classified as archived
**And**: analysis does not fail with `Missing dependency reference` for `base`

#### Scenario: Missing dependency remains fail-closed

**Given**: the target repository has no active, in-flight, archived, or rejected change matching dependency `ghost`
**And**: an active queued change `dependent` declares dependency `ghost`
**When**: dependency analysis validates `dependent`
**Then**: analysis fails closed with a dedicated missing-dependency diagnostic for `ghost`

#### Scenario: Rejected dependency remains fail-closed

**Given**: the target repository contains active change `base` with `proposal.md` and `REJECTED.md`
**And**: an active queued change `dependent` declares dependency `base`
**When**: dependency analysis validates `dependent`
**Then**: analysis fails closed with a dedicated rejected-dependency diagnostic for `base`
