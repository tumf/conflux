# Design

## Authority

The evidence envelope is a repository-local runtime sidecar under the change worktree. It is excluded from Git and from the clean-state comparison described below, but it remains inspectable and survives a Conflux process restart. It is evidence, not hidden workflow state. Deleting external Conflux state does not change the decision.

Only a Conflux-runtime-owned verification executor may create a reusable envelope. The executor directly starts and waits for the declared argv, captures the exit status and output artifact, resolves the executable, reads Git identities, and atomically writes the envelope. Apply may request an execution, but an envelope written or edited by the Apply agent is not eligible. Runtime tests use injected Git and process adapters; production does not trust agent-authored result fields.

## Fail-closed validator

Validation is an all-fields conjunction. No partial score or freshness heuristic exists. A mismatch selects the existing command-rerun path. It never converts an implementation result into PASS.

## Identity choices

Use full Git object IDs. Bind tracked automation by blob ID. Record argv as an array, not a shell string. Resolve the executable and bind an immutable digest; where that is unavailable, require exact version output plus the executable file digest. Hash every reused artifact.

The bound source identity is the full-length Apply commit object ID plus its tree ID. At capture and reuse, the index and worktree MUST have no changes other than the designated runtime evidence directory. This evidence-path exclusion avoids self-reference while ensuring source, declaration, automation, and executable changes invalidate reuse. A sidecar is never added to the Apply commit.

## Capture boundary

The runtime executor snapshots the commit/tree, declaration, argv, cwd, automation blob, executable identity, and evidence-path-excluded clean state before execution. It writes the envelope only after successful process exit, artifact hashing, a second evidence-path-excluded clean-state check, and confirmation that every pre-execution binding remains unchanged. Atomic file replacement prevents partially written records from becoming candidates. Directly authored, malformed, partial, or unsuccessful records only select rerun.

## Cheap checks

A repository-tracked policy in the Acceptance runtime names the minimum elapsed duration eligible for reuse and defaults to rerunning commands below that threshold. The runtime measures elapsed duration itself; proposal prose or agent claims cannot override it. This optimization never weakens correctness.
