## MODIFIED Requirements

### Requirement: Reducer Input Precedence and Idempotency

Execution events SHALL own active-stage and terminal transitions. Workspace observations SHALL reconcile durable wait/recovery state and MUST NOT override an active execution stage.

When a workspace-status synchronization event is used, it SHALL target the specific change being updated. The reducer MUST NOT infer the target change from `current_change_id` when multiple changes may be active concurrently.

#### Scenario: Workspace status sync targets the correct change

- **GIVEN** change `a` is `Applying`
- **AND** change `b` is `Rejecting`
- **WHEN** a workspace-status synchronization event for `b` is applied
- **THEN** only change `b` is updated
- **AND** change `a` remains `Applying`

#### Scenario: Parallel workspace status sync does not rely on current change id

- **GIVEN** multiple changes are active in parallel mode
- **WHEN** a workspace-status synchronization event arrives
- **THEN** the reducer identifies the target change from the event payload itself
- **AND** `current_change_id` is not used to decide which runtime entry to mutate
