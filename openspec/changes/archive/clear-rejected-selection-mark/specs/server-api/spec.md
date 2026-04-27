## MODIFIED Requirements

### Requirement: change-selection-state

The server SHALL manage in-memory `selected: bool` state for each project change.

A change that is represented as terminal `rejected` in server-facing snapshots or state transitions MUST NOT remain selected. When rejection is confirmed, the server-visible selection state for that change SHALL be cleared to `selected = false`.

#### Scenario: rejected change selection is cleared in server state

**Given**: change `foo` is `selected: true`
**And**: the system confirms rejection for change `foo`
**When**: the server updates its change state snapshot
**Then**: change `foo` is represented as `selected: false`
**And**: unrelated changes keep their previous `selected` values

### Requirement: dashboard-change-checkbox

The dashboard SHALL display a checkbox for change rows that participate in normal selection semantics.

Rejected terminal rows MAY remain visible for read-only operational visibility, but they MUST NOT remain represented as selected execution candidates.

#### Scenario: rejected row is not kept as selected

**Given**: the dashboard renders a change row whose status is `rejected`
**When**: the row is built from the latest server snapshot
**Then**: that row is represented with `selected: false`
**And**: it is not treated as an active execution candidate for global Run
