## ADDED Requirements

### Requirement: Target-scoped force-stop API and clients

The v2 command registry MUST accept `force_stop_change` with exactly one non-empty `change_id`, ordinary expected-revision fencing, and idempotency identity. Each projected change MUST publish `actions.force_stop_change` eligibility. A successful settled command result MUST identify the target, cancelled phase, last completed phase, confirmed termination, and `effects_rolled_back: false`.

The CLI MUST expose `cflx client force-stop-change <change-id> --json`. MCP `cflx_control` MUST accept action `force_stop_change` with exactly one distinct `change_id`. Both clients MUST delegate through the ordinary v2 command and shared operator transaction; they MUST NOT invoke process-wide `force_stop`, rewrite marks, perform PID lookup, or reimplement cancellation policy.

#### Scenario: API addresses one change

- **GIVEN** the owner publishes `force_stop_change` eligibility for `alpha`
- **WHEN** the caller submits `force_stop_change` for `alpha` at the current revision
- **THEN** one ordinary command record is created for `alpha`
- **AND** settlement returns the target-specific termination result
- **AND** no process-wide ForceStop command is created

#### Scenario: Per-change eligibility is authoritative

- **GIVEN** `alpha` can be individually force-stopped and `beta` cannot
- **WHEN** clients inspect state
- **THEN** `alpha.actions.force_stop_change` is allowed
- **AND** `beta.actions.force_stop_change` carries a typed blocked reason

#### Scenario: CLI requires exactly one target

- **WHEN** `cflx client force-stop-change` receives zero or multiple change IDs
- **THEN** it returns a typed usage error before contacting the owner
- **AND** no command record is created

#### Scenario: MCP requires exactly one target

- **WHEN** `cflx_control` action `force_stop_change` receives zero, multiple, duplicate, or blank change IDs
- **THEN** MCP validation refuses the call before mutation
- **AND** no lifecycle command is submitted

#### Scenario: Process-wide force-stop remains distinct

- **WHEN** a caller invokes existing `force_stop`
- **THEN** its process-wide semantics and target-list refusal remain unchanged
- **AND** `force_stop_change` remains the only immediate target-scoped kill operation
