## MODIFIED Requirements

### Requirement: Queue State Synchronization

The system SHALL always synchronize the queue state displayed in the UI with the DynamicQueue state.

`ResolveWait` is a state waiting for scheduler-owned resolve retry work, and Space queue operations MUST NOT modify DynamicQueue. `MergeWait` similarly MUST NOT be a target for normal queue operations.

However, for `ResolveWait`/`MergeWait` rows, the following SHALL be satisfied:
- Space operation SHALL toggle only the execution mark (`selected`) and MUST NOT modify `queue_status` or DynamicQueue.
- @ operation SHALL be ignored and MUST NOT modify any state.

The TUI MUST display `ResolveWait` as `resolve pending` to clearly indicate it is scheduler-owned retry work and not normal queue work.

In parallel mode, once the user explicitly queues a `NotQueued` change for execution (for example via the configured start key after marking it), refresh-derived state reconciliation MUST preserve the queued display state until one of the following occurs:
- execution for that change actually starts,
- the backend explicitly rejects startup for that change, or
- the user explicitly dequeues the change.

Auto-refresh, reducer display synchronization, and eligibility reconciliation MUST NOT regress such a queued row back to `not queued` before backend analysis/dispatch begins.

The configured start keybindings SHALL be treated as app-level orchestration control and MUST NOT perform cursor-local merge resolve actions. A cursor row in `MergeWait` MUST NOT cause any configured start key to emit `ResolveMerge` or transition that row to `resolve pending`.

<!-- Expected canonical result after archive: `tui-architecture` will generalize the historical F5-specific orchestration control rule to all configured start keybindings. -->

#### Scenario: Configured start key on MergeWait does not resolve cursor row

- **GIVEN** the TUI cursor is on change `alpha`
- **AND** `alpha` is in `MergeWait`
- **AND** change `beta` is marked runnable work in `NotQueued`
- **AND** the resolved TUI start keybindings include `r`
- **WHEN** the user presses `r`
- **THEN** the TUI SHALL NOT emit `ResolveMerge(alpha)`
- **AND** `alpha` SHALL NOT transition to `resolve pending` because of `r`
- **AND** normal orchestration start/resume/retry MAY proceed for marked runnable work such as `beta`

#### Scenario: Default F5 remains an app-level start key

- **GIVEN** no TUI config override exists
- **AND** the TUI cursor is on change `alpha` in `MergeWait`
- **WHEN** the user presses `F5`
- **THEN** the TUI SHALL NOT emit `ResolveMerge(alpha)`
- **AND** normal orchestration start/resume/retry MAY proceed for marked runnable work
