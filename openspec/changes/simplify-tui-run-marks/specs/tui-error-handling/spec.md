## MODIFIED Requirements

### Requirement: Change-Level Processing Errors Do Not Force App Error Mode

When a change emits `ProcessingError`, Conflux SHALL record the target change as change-level `Error`, retain its diagnostic evidence, and revoke stale execution intent according to the existing change-level reconciliation rules. The event SHALL NOT transition the process-wide Core or TUI execution mode to `Error`. Every process-mode projection SHALL preserve the mode that existed before the event; only a typed fatal global error may enter process-wide Error.

A later Ready/Select or Stopped projection MUST NOT make that recovery unreachable. After the operator re-marks a retry-eligible change-level Error row, configured Start/F5 SHALL route it through the existing typed retry path based on current target evidence rather than requiring process-wide Error mode. Marking alone SHALL remain side-effect free.

Non-fatal warning popups used for merge, resolve, hook, and warning diagnostics SHALL preserve readable diagnostic content. When a warning popup message contains explicit newlines, the popup SHALL preserve those line boundaries. When warning popup content exceeds the visible body area, the TUI SHALL provide popup-local scrolling and SHALL NOT route popup keys to an interaction modal, underlying change list, worktree list, or log panel. Warning popup presentation state SHALL remain independent from execution and interaction-modal state and SHALL NOT be used as workflow-control input.

#### Scenario: One change fails while unrelated work remains active

- **GIVEN** the authoritative process mode and TUI execution mode are `Running`
- **AND** changes `alpha` and `beta` are eligible participants in the run
- **WHEN** `ProcessingError` is dispatched for `alpha`
- **THEN** `alpha` SHALL transition to change-level `Error` with retained diagnostic evidence
- **AND** `alpha`'s stale execution mark SHALL be revoked
- **AND** the authoritative process mode and TUI execution mode SHALL remain `Running`
- **AND** unrelated eligible mark state SHALL remain independently mutable

#### Scenario: Ready re-mark and F5 reaches typed retry

- **GIVEN** `ProcessingError` moved `alpha` to change-level `Error`
- **AND** the persistent scheduler later projects Ready/Select
- **WHEN** the operator re-marks `alpha` and presses configured Start/F5
- **THEN** `alpha` SHALL be routed through its typed retry path
- **AND** Core and TUI SHALL NOT need to manufacture process-wide Error mode

#### Scenario: Marking recovery intent has no immediate effect

- **GIVEN** `alpha` is a visible retry-eligible change-level Error row
- **WHEN** the operator marks `alpha`
- **THEN** only its process-local execution mark changes
- **AND** no retry, queue, scheduler, or process-mode effect occurs until configured Start/F5 is submitted

<!-- Expected canonical result after archive: `tui-error-handling` will keep ProcessingError change-scoped while guaranteeing that Ready/Select re-mark plus configured Start/F5 can still reach typed retry. -->
