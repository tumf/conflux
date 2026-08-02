## Implementation Tasks

- [x] Extend the coherent v2 state/change DTOs with execution mark, queue intent, NEW attention, blocker kind/detail, change-local error detail, action eligibility/reasons, parallel eligibility, timing, latest activity, and worktree relation while preserving process-local/non-durable semantics. (verification: unit - `cargo test --features web-monitoring remote_control_api` verifies DTO serialization and restart-default tests prove every field and ephemeral reset behavior; verification-id: authoritative-snapshot-tests)

- [x] Wire reducer, operator intent store, log projection, and worktree observation into one snapshot revision without deriving values from display strings or frontend state. (verification: integration - `cargo test --features web-monitoring remote_control_api` verifies projection fixtures prove coherent blocked/stalled/error/active/final/worktree-linked snapshots and no-op revision behavior; verification-id: authoritative-snapshot-tests)

- [x] Publish ordered state updates whenever an in-scope decision field changes and restore the complete state after replay gaps or process-incarnation changes. (verification: integration - `cargo test --features web-monitoring remote_control_api` verifies replay/gap tests replace the snapshot and recover marks, queue intent, eligibility, details, timing, activity, and relations without replay inference; verification-id: authoritative-snapshot-tests)

- [x] Add API and OpenAPI schema coverage for absent values, sanitization, secret/path redaction, mutation readback, and all canonical status/blocker/action combinations. (verification: integration - `cargo test --features web-monitoring remote_control_api` verifies authenticated state/change route tests and schema assertions fail on omitted or leaked fields; verification-id: authoritative-snapshot-tests)

## Implementation Notes

New wire surface on every projected change: `execution_marked`, `queue_intent`,
`attention`, `blocker` (with machine-readable `kind`), `error_detail`, `actions`
(per-command `allowed` plus a stable `blocked_reason` token), `parallel`,
`timing`, `latest_activity`, and `worktree`. `InstanceSnapshot` gained
`process_error` so a fatal run failure stays distinguishable from a change-local
one. Absent values serialize as explicit `null`s rather than omitted keys, so one
snapshot can clear a field a client already holds.

Action eligibility is derived from the same lifecycle matrix the TUI uses
(`classify_mark_route` / `classify_retry_route` in
`src/orchestration/operator_command.rs`), so a remote controller and a keypress
cannot be offered different actions. `resolve_merge` is deliberately stricter
than the reducer: it is advertised only for `merge wait`, `resolve pending`, and
`archived`, so nothing reported as `allowed` can fail its guards.

Process-local, non-durable state lives in `src/web/operator_facts.rs` (timing,
latest activity, attention, parallel eligibility, worktree relation) and in the
existing `ExecutionMarkStore`. A restart starts empty by construction and routing
is recomputed from the workspace, as `openspec/CONSTITUTION.md` requires.

Two ordering fixes were required for the snapshot to be authoritative:
`WebState::operator_snapshot` now reads reducer-derived status on every
projection rather than only on event paths that set `updated` (an acceptance hold
changes no monitoring field of its own), and `SharedServiceExecutor::execute`
publishes the projection before a command record settles, since execution marks
and queue intent emit no execution event of their own.

Latest activity excludes streaming output, progress ticks, and logs: including
them would advance `state_revision` on every chunk and leave every client's
optimistic-concurrency token permanently stale.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate expose-authoritative-operator-snapshot --archive-gate`

The implementation must also pass `cargo test --features web-monitoring remote_control_api`.

## Future Work

- Browser presentation and interaction remain in the consuming Web project or `modernize-web-monitoring-ui`.

## Acceptance Repair Notes

Attempt 1 repaired `acceptance-openapi-artifact-emptied` by regenerating
`docs/openapi.yaml` from `openapi-gen` (1565 lines, 48450 bytes), restoring the
artifact this change's own `docs/guides/WEBUI.md` and `docs/guides/WEBUI.ja.md`
cite as the complete API reference.

`Makefile` was also changed, and its relationship to that finding is causal: the
`openapi` target used `cargo run ... > docs/openapi.yaml`, so the shell truncated
the checked-in artifact *before* the generator ran and left it at 0 bytes whenever
the build failed — the exact way the apply commit emptied it. The target now
writes to `/tmp/openapi-gen.yaml` and only `mv`s it into `docs/` on success, which
mirrors what `check-openapi` already did and makes the finding non-recurring.

## Current Acceptance Follow-up
- attempt: 1
- [x] [acceptance-openapi-artifact-emptied] (minor) The checked-in generated OpenAPI artifact docs/openapi.yaml was emptied (1281 lines on main to 0 bytes) by the apply commit, while this change's own docs cite it as the complete API reference and make check-openapi fails against it | evidence: git show main:docs/openapi.yaml | wc -l -> 1281; git show HEAD:docs/openapi.yaml | wc -c -> 0 (emptied in commit 6d546930 'Apply: expose-authoritative-operator-snapshot'); docs/guides/WEBUI.md:61 and docs/guides/WEBUI.ja.md:61 (both edited by this change) plus docs/guides/USAGE.md:172 direct readers to docs/openapi.yaml for complete API details; Makefile target check-openapi diffs openapi-gen output against docs/openapi.yaml and exits 1; regeneration verified working: cargo run --bin openapi-gen --features web-monitoring exits 0 and emits 1565 lines including QueueIntent, ChangeActions, ChangeBlocker, ParallelEligibility, ChangeWorktree, and process_error; scripts/release.sh:173-189 stamps and git-adds docs/openapi.yaml during release, so the next release would publish an empty spec artifact | required_changes: docs/openapi.yaml — Regenerate the artifact with make openapi (cargo run --bin openapi-gen --features web-monitoring > docs/openapi.yaml) and commit the non-empty spec including the new authoritative-snapshot schemas | verification: docs/openapi.yaml — make check-openapi exits 0 (checked-in file identical to regenerated output); the file is non-empty and describes QueueIntent, ChangeActions, ChangeBlocker, ParallelEligibility, ChangeWorktree, and InstanceSnapshot.process_error
  finding: {"evidence":["git show main:docs/openapi.yaml | wc -l -> 1281; git show HEAD:docs/openapi.yaml | wc -c -> 0 (emptied in commit 6d546930 'Apply: expose-authoritative-operator-snapshot')","docs/guides/WEBUI.md:61 and docs/guides/WEBUI.ja.md:61 (both edited by this change) plus docs/guides/USAGE.md:172 direct readers to docs/openapi.yaml for complete API details","Makefile target check-openapi diffs openapi-gen output against docs/openapi.yaml and exits 1; regeneration verified working: cargo run --bin openapi-gen --features web-monitoring exits 0 and emits 1565 lines including QueueIntent, ChangeActions, ChangeBlocker, ParallelEligibility, ChangeWorktree, and process_error","scripts/release.sh:173-189 stamps and git-adds docs/openapi.yaml during release, so the next release would publish an empty spec artifact"],"id":"acceptance-openapi-artifact-emptied","required_changes":[{"description":"Regenerate the artifact with make openapi (cargo run --bin openapi-gen --features web-monitoring > docs/openapi.yaml) and commit the non-empty spec including the new authoritative-snapshot schemas","file":"docs/openapi.yaml"}],"severity":"minor","summary":"The checked-in generated OpenAPI artifact docs/openapi.yaml was emptied (1281 lines on main to 0 bytes) by the apply commit, while this change's own docs cite it as the complete API reference and make check-openapi fails against it","verification":[{"description":"make check-openapi exits 0 (checked-in file identical to regenerated output); the file is non-empty and describes QueueIntent, ChangeActions, ChangeBlocker, ParallelEligibility, ChangeWorktree, and InstanceSnapshot.process_error","file":"docs/openapi.yaml"}]}
  evidence: required_changes docs/openapi.yaml — `make openapi` regenerated the artifact from `openapi-gen`: 0 bytes -> 1565 lines / 48450 bytes, committed non-empty
  evidence: verification docs/openapi.yaml — `make check-openapi` exits 0 ("OpenAPI specification is up to date."), so the checked-in file is byte-identical to regenerated output
  evidence: verification docs/openapi.yaml — grep confirms QueueIntent, ChangeActions, ChangeBlocker, ParallelEligibility and ChangeWorktree schemas plus `process_error` at line 1189 inside `InstanceSnapshot` (line 1164)
  evidence: root cause — `Makefile` `openapi` target now generates to /tmp and `mv`s on success, so a failed build can no longer truncate docs/openapi.yaml to 0 bytes
