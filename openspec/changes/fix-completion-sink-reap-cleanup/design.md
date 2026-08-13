# Design

## Cleanup acknowledgement

The finite shutdown deadline bounds graceful callback execution, not the correctness of reap ordering. Once it expires, cancellation forces the active child through explicit kill-and-wait. Event cleanup then waits for the dispatcher acknowledgement proving that path settled. A second timeout that deletes artifacts without acknowledgement is forbidden because it recreates the live-callback cleanup race.

The regression test injects a synchronization barrier around cancellation/reap acknowledgement. It asserts the artifact exists before acknowledgement and is removed only after acknowledgement, without treating a short wall-clock threshold as correctness evidence.
