---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/operator-command-execution/spec.md
  - src/parallel/orchestration.rs
verifications:
  - id: blocked-idle-graceful-stop
    requirement: Accepted graceful stop settles an inactive blocked-only scheduler without executing or destroying pending work
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/orchestration.rs
    evidence: cargo test --locked --offline --lib --features web-monitoring blocked_idle_graceful_stop -- --nocapture
    rerun: cargo test --locked --offline --lib --features web-monitoring blocked_idle_graceful_stop -- --nocapture
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Settle graceful Stop at a blocked-only idle boundary

**Change Type**: implementation

## Problem and observed evidence

On fixed f618a3e7 a fresh real TUI owner with valid CRLF proposal metadata and one missing dependency parks with `scheduler_running=true`, `has_active_work=false`, `persistent_scheduler_idle=true`. `client stop` is accepted, its command succeeds and TUI/API project `stopping`, but the scheduler parks again because `settle_graceful_stop_when_no_work` requires an empty local queue. No process is executing and no future work boundary can arrive until an unrelated action. Original evidence is in the parent-owned `/home/tumf/work/acceptance-checks/residual-fixes/stop-before-f618a3e7/evidence` directory. That path is evidence, not a gate dependency or a user configuration requirement.

## Selected behavior

An accepted graceful stop must settle at a coherent inactive blocked-only scheduler boundary as well as at the existing fully-drained boundary. A queued dependency-blocked, change-local terminal/error, or inactive Acceptance-held target is not an executing phase whose completion should be awaited. Detect this through the scheduler's existing coherent reducer snapshot and blocked-only eligibility authority; do not infer it solely from presentation flags or a failed lock probe.

Execution marks, repository/worktree effects and real blocker/Acceptance/error evidence are preserved; queue intent follows existing process-wide Stopped reconciliation (ordinary queued rows return to `not queued`, never to the terminal `stopped` row of `force_stop_change`, never to completed). Do not silently execute blocked work, retry it, clear its blockers, archive it, or report success. Reuse the normal exactly-once `Stopped` event and bounded cleanup barrier, with no new persistent authority, status, config, timeout setting, command or frontend-specific shortcut. The shared lifecycle projections must leave Stopping and report an inactive scheduler; pending/blocked changes must not become completed. Subsequent explicit Start/Retry uses existing admission semantics; a mark/file edit alone must not restart stopped work.

## Safety boundary

- Coherent snapshot must positively prove no active/admitted execution, no in-flight JoinSet task, manual resolver, pending merge/push or cleanup obligation, and no resolve/reject lane waiter owed the existing boundary. Incomplete reducer evidence is fail-closed and must not settle.
- A process-level idle boolean, queue-only snapshot or missing execution handle alone is insufficient proof. Do not settle early during Apply/Acceptance/archive/resolve/merge or throw away pending cleanup. Preserve active-work graceful-stop behavior and cancel-stop races.
- Use existing operator-stop outcome/dequeue policy; do not invent terminal success, erase held evidence or introduce automatic readmission. Preserve genuine dependency auto-recovery when no stop is pending.
- Limit edits to scheduler stop/idle boundaries and directly necessary lifecycle tests. No TUI layout/color/key changes, no live owner replacement, source installation, private candidate change, push or release.

## Acceptance

1. Deterministic full persistent scheduler-loop test: valid proposal queued behind missing dependency, no managed work; accepted graceful stop wakes the existing park and settles to one Stopped within a bounded test deadline without extra external wake or wall-clock sleep. No analyzer/apply/gate/reviewer invocation, no completed success, no busy loop.
2. Positive settlement is bounded by the existing blocked-only classification; this change MUST NOT widen `is_blocked_only` to admit new row kinds. Inactive Acceptance-hold/error controls preserve evidence/marks under normal stop reconciliation: rows not classified blocked-only are either already not queued (drained boundary applies) or dispatchable (no settlement). Existing fully empty stop remains valid.
3. Negative controls with active task, incomplete snapshot, resolve/reject waiter, manual resolve or pending merge do not settle early; once genuine work finishes its established boundary, the stop settles exactly once. Cancel-stop before settlement withdraws the stop and preserves existing Ready/Running projection semantics; this change does not independently redesign persistent-idle projection. A cancel-stop accepted before boundary settlement must prevent Stopped from that withdrawn request.
4. Shared lifecycle/API/TUI TestBackend projections agree on leaving Stopping and scheduler inactivity. `client wait` MUST NOT return completed for a stopped run merely because Stop returned 0. Post-stop ordinary row classification is existing not-queued-with-mark behavior and wait semantics on that row are unchanged; releasing that wait is not in this change. Explicit subsequent Start/Retry preserves blocked/held behavior and does not re-execute Apply already completed.
5. All change-blocking tests live under src/ module tests and include `blocked_idle_graceful_stop` in their names; the declared command selects nonzero cases. Tests assert final state and event/execution counts with deterministic barriers, not timeout-only PASS or manual Ctrl+C. Parent separately repeats a disposable real-owner TUI/API case on the fixed build and distinguishes synthetic workers from AI validation.

## Non-goals

Do not change whole-product stop semantics, force stop scope or cleanup deadlines, add optional workaround settings, or treat a successful wait process as implementation acceptance. A proposal-diagnostic repair is a separate change; this fix must also work for a genuinely dependency-blocked valid proposal.
