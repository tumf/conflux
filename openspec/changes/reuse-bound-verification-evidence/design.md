# Design

## Authority

The evidence envelope is tracked in the change worktree and validated against current Git. It is evidence, not hidden workflow state. Deleting external Conflux state does not change the decision.

## Fail-closed validator

Validation is an all-fields conjunction. No partial score or freshness heuristic exists. A mismatch selects the existing command-rerun path. It never converts an implementation result into PASS.

## Identity choices

Use full Git object IDs. Bind tracked automation by blob ID. Record argv as an array, not a shell string. Resolve the executable and bind an immutable digest; where that is unavailable, require exact version output plus the executable file digest. Hash every reused artifact.

## Capture boundary

Write the envelope only after successful process exit and artifact hashing, then prove clean index/worktree state. Atomic file replacement prevents partially written records from becoming candidates.

## Cheap checks

A small explicit policy keeps cheap validations rerunnable. Reuse initially targets commands whose measured or declared cost justifies avoiding exact duplication; this optimization never weakens correctness.
