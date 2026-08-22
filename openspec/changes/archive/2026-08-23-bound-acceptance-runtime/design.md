# Design

## Decision

Use the existing command-runner absolute deadline and process-group cleanup machinery. Add only an Acceptance-specific configured value and route it at the Acceptance call site.

## Semantics

`command_max_runtime_secs` remains the general safety budget. Acceptance is a reviewer, not an implementation worker, so it receives a non-disableable dedicated budget. When the common limit is a positive value, Acceptance uses the minimum of the common and dedicated limits; when the common limit is zero, the dedicated limit still bounds Acceptance. Runtime expiry is a typed terminal result for that invocation. It never becomes PASS, inactivity timeout, protocol continuation, corrective retry, or automatic resampling.

## Recovery

The repository and worktree remain authoritative. An operator may explicitly retry after correcting proposal scope or environment. No durable timeout state outside the workspace is introduced.
