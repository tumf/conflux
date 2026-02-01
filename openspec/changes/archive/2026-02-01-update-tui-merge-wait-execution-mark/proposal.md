# Change: Allow toggling execution mark in TUI merge/resolve wait states

## Why
There are cases where the TUI displays `Ready` but internally remains in `Running` state, preventing the removal of the execution mark (`[x]`) on `MergeWait`/`ResolveWait` rows. To organize wait states and enable re-execution decisions, toggling the execution mark needs to be allowed.

## What Changes
- Allow Space operation to toggle the execution mark on `MergeWait`/`ResolveWait` rows
- Allow @ operation to toggle the approval state on `MergeWait`/`ResolveWait` rows (without changing queue status or DynamicQueue)
- Maintain the prohibition of DynamicQueue changes via Space/@ (wait states are not queue operation targets)
- Preserve existing queue operation behavior (`NotQueued`/`Queued` add/remove)

## Impact
- Affected specs: `tui-architecture`
- Affected code: `src/tui/state/guards.rs`, `src/tui/state/mod.rs`
