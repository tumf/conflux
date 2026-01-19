# Change: merge_wait does not stop orchestration

## Why
In parallel execution, when a change enters MergeWait, orchestration should continue to process other runnable changes. Currently, orchestration can stop early or treat MergeWait as a terminal success condition, which blocks remaining work.

## What Changes
- Clarify that MergeWait is not a completion reason and must not halt orchestration loops.
- Ensure runnable queued changes continue to execute even if other changes are in MergeWait.
- Prevent success completion events/messages when MergeWait remains.

## Impact
- Affected specs: parallel-execution
- Affected code: parallel execution loop and completion handling
