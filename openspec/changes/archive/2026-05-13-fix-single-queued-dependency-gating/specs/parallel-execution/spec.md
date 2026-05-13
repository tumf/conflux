## MODIFIED Requirements

### Requirement: archived dependency references have explicit scheduler and validation semantics

The system SHALL classify dependency targets referenced from active change metadata into at least queued, in-flight, active-but-not-queued, archived, rejected, and missing categories.

Proposal metadata dependencies SHALL be treated as authoritative hard dependencies by analyzer and scheduler paths. LLM analysis MAY add valid required dependency edges, but it MUST NOT remove or silently ignore dependencies parsed from proposal frontmatter or body fallback metadata. When LLM analysis is skipped for a single queued change, the analyzer MUST still return metadata dependencies in the normalized analysis result.

Queued, in-flight, and active-but-not-queued dependency targets SHALL participate in dispatch gating and MUST prevent dependent changes from starting until the dependency is resolved on the base branch. Archived dependency targets SHALL be treated as already satisfied references and MUST NOT block dispatch, trigger terminal rejection, or be surfaced as generic JSON/parse failures. Rejected and missing dependency targets SHALL fail closed with dedicated diagnostics and MUST NOT allow the dependent change to dispatch.

#### Scenario: metadata dependency blocks while dependency is applying

- **GIVEN** active change `route` has proposal metadata dependency `policy`
- **AND** `policy` is currently in-flight applying
- **WHEN** analyzer and scheduler evaluate `route`
- **THEN** `route` remains dependency-blocked
- **AND** `route` is not dispatched to apply
- **AND** the dependency diagnostic identifies `policy` as in-flight or unresolved rather than omitting the edge

#### Scenario: single queued change preserves metadata dependency

- **GIVEN** `route` is the only queued change
- **AND** `route` has proposal metadata dependency `policy`
- **WHEN** analyzer uses a single-change fast path
- **THEN** the analysis result still contains `route -> policy`
- **AND** scheduler applies normal dependency gating before dispatching `route`

#### Scenario: single queued change blocks on active dependency outside queue

- **GIVEN** `route` is the only queued change
- **AND** `route` has proposal metadata dependency `policy`
- **AND** `policy` exists as an active change under `openspec/changes/policy/`
- **AND** `policy` is not queued, not in-flight, not archived, and not merged to the base branch
- **WHEN** scheduler evaluates dispatch eligibility for `route`
- **THEN** `route` remains dependency-blocked
- **AND** `route` is not dispatched to apply
- **AND** no `ApplyStarted` event is emitted for `route`
- **AND** the dependency diagnostic identifies `policy` as the unresolved blocker

#### Scenario: single queued change may dispatch after archived dependency is satisfied

- **GIVEN** `route` is the only queued change
- **AND** `route` has proposal metadata dependency `policy`
- **AND** `policy` exists under `openspec/changes/archive/` with either exact or date-prefixed archive directory naming
- **WHEN** scheduler evaluates dispatch eligibility for `route`
- **THEN** `policy` is treated as satisfied
- **AND** `route` may be dispatched when it has no other unresolved dependencies

#### Scenario: single queued change fails closed on missing dependency

- **GIVEN** `route` is the only queued change
- **AND** `route` has proposal metadata dependency `ghost`
- **AND** `ghost` exists neither in the queued set, nor the in-flight set, nor active changes, nor the archive tree
- **WHEN** scheduler evaluates dispatch eligibility for `route`
- **THEN** `ghost` is classified as missing
- **AND** `route` is not dispatched
- **AND** the diagnostic distinguishes missing dependency from archived or active dependency

#### Scenario: fallback analysis preserves metadata dependency

- **GIVEN** `route` has proposal metadata dependency `policy`
- **AND** LLM analysis fails or is disabled
- **WHEN** fallback analysis creates an order result
- **THEN** the fallback result includes `route -> policy`
- **AND** the fallback is metadata-dependency-only rather than dependency-free

#### Scenario: archived dependency is satisfied and not rejected

- **GIVEN** active change `route` references dependency `contracts`
- **AND** `contracts` exists under `openspec/changes/archive/` with either exact or date-prefixed archive directory naming
- **WHEN** analyzer validation and scheduler dispatch checks evaluate `route`
- **THEN** `contracts` is classified as archived
- **AND** `route` is not rejected because of `contracts`
- **AND** `contracts` does not block dispatch once all non-archived dependencies are resolved
- **AND** diagnostics do not collapse the condition into generic invalid JSON or missing dependency output

#### Scenario: missing dependency fails closed

- **GIVEN** active change `route` references dependency `ghost`
- **AND** `ghost` exists neither in the queued set, nor the in-flight set, nor the archive tree
- **WHEN** analyzer validation or scheduler dispatch checks evaluate `route`
- **THEN** `ghost` is classified as missing
- **AND** `route` is not dispatched
- **AND** the diagnostic distinguishes missing dependency from archived dependency

#### Scenario: LLM cannot remove metadata dependency

- **GIVEN** active change `route` has proposal metadata dependency `policy`
- **AND** LLM analysis returns dependencies that omit `policy`
- **WHEN** Conflux parses and normalizes the analysis result
- **THEN** the normalized dependencies still include `route -> policy`
- **AND** dispatch gating uses the normalized dependency set
