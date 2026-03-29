## Requirements

### Requirement: resolve-merge-reducer-sync

When a user triggers merge resolve (`M` key) on a `MergeWait` change, the shared orchestration reducer MUST be updated with `ResolveMerge` intent regardless of whether resolve executes immediately or is queued.

#### Scenario: immediate-resolve-syncs-reducer

**Given**: A change is in `MergeWait` state and no other resolve is in progress (`is_resolving == false`)
**When**: The user presses `M` to trigger resolve
**Then**: The shared reducer transitions the change to `ResolveWait`, and subsequent `ChangesRefreshed` display syncs preserve `ResolveWait` (not regress to `MergeWait`)

#### Scenario: queued-resolve-syncs-reducer

**Given**: A change is in `MergeWait` state and another resolve is already in progress (`is_resolving == true`)
**When**: The user presses `M` to queue resolve
**Then**: The shared reducer transitions the change to `ResolveWait`, and subsequent `ChangesRefreshed` display syncs preserve `ResolveWait`
