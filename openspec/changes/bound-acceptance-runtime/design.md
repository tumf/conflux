# Design

## Decision

Use the existing command-runner absolute deadline and process-group cleanup machinery. Add only an Acceptance-specific configured value and route it at the Acceptance call site.

## Semantics

`command_max_runtime_secs` remains the general budget. Acceptance is a reviewer, not an implementation worker, so it receives a non-disableable shorter budget. Runtime expiry is a typed terminal result for that invocation. It never becomes PASS and never triggers automatic resampling.

## Recovery

The repository and worktree remain authoritative. An operator may explicitly retry after correcting proposal scope or environment. No durable timeout state outside the workspace is introduced.
