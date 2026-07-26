## ADDED Requirements

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
