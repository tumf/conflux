## MODIFIED Requirements

### Requirement: resolve-merge-queue-persistence

When a user requests merge resolution (`M` key) on a `MergeWait` change while another resolve is in progress, the change must transition to `ResolveWait` and remain in that state until the resolve is actually started or explicitly cancelled. The transition must be synchronized to both the TUI-local state and the shared orchestrator reducer.

#### Scenario: queued-resolve-survives-refresh

**Given**: Change A is in `Resolving` state and Change B is in `MergeWait` state in the TUI
**When**: The user presses `M` on Change B, then a `ChangesRefreshed` event fires with Change B's workspace still in `Archived` state
**Then**: Change B remains in `ResolveWait` ("resolve pending") in both the TUI display and the shared reducer state

#### Scenario: queued-resolve-eventually-executes

**Given**: Change B has been queued for resolve via `M` key while Change A was resolving
**When**: Change A's resolve completes
**Then**: Change B's resolve is started from the queue
