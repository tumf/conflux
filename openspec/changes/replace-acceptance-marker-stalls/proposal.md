---
change_type: hybrid
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/changes/archive/2026-07-21-persist-acceptance-stalled-state/
  - openspec/changes/archive/2026-07-26-eliminate-acceptance-checkpoint/
  - openspec/changes/archive/2026-07-26-resume-missing-acceptance-verdict/
  - src/acceptance.rs
  - src/events.rs
  - src/execution/state.rs
  - src/orchestration/acceptance.rs
  - src/orchestration/state.rs
  - src/parallel/acceptance_state.rs
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - src/serial_run_service.rs
  - skills/cflx-accept/SKILL.md
verifications:
  - id: acceptance-stall-state-local
    requirement: Bare GATED is bounded protocol failure while validated external blockers use revision-bound runtime state without dirtying the managed worktree
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: focused parser, retry, runtime-state, restart, migration, queue, and clean-worktree test output recorded in acceptance review
    rerun: cargo test acceptance
    prerequisites: []
---

# Replace Acceptance Marker Stalls

**Change Type**: hybrid

## Problem / Context

Acceptance currently reduces both `{"acceptance":"gated"}` and `ACCEPTANCE: GATED` to an evidence-free `AcceptanceResult::Gated`. Parallel and serial orchestration then create `openspec/changes/<change-id>/APPLY_BLOCKED/marker.md`, even when no dedicated blocker artifact or concrete external prerequisite exists.

The marker is workflow-generated repository content. It makes a previously clean managed worktree dirty, is detected before the existing Apply commit, changes workspace state to `Blocked`, and requires explicit retry to consume the marker before Acceptance can resume. A consume or queue-routing defect can therefore re-submit the same change from Blocked to Blocked. Runtime retry metadata and inferred blocker categories also leak into the OpenSpec change artifact tree.

The current Constitution requires all workflow routing to be derivable from workspace state and forbids authoritative out-of-worktree durable state. The requested runtime-state design intentionally changes that law. This change therefore amends `openspec/CONSTITUTION.md` atomically with the runtime, prompt, migration, and specification changes. Truthful completion remains repository-evidence based: runtime stall state may pause or resume execution, but can never prove implementation, Acceptance PASS, archive readiness, or merge completion.

## Proposed Solution

Distinguish a bare compatibility verdict from a validated stalled blocker. A bare `gated` or legacy `blocked` verdict without structured blocker evidence is an Acceptance protocol error. Reuse the existing missing-verdict retry policy: retry Acceptance at most twice through the normal configured command, do not rerun Apply, and return a terminal protocol error after exhaustion. Bare input creates neither a stalled lifecycle event nor a blocker category, marker, or durable stall record.

Require a structured blocker payload before entering `stalled`. The payload must provide an explicit category, non-empty concrete evidence, next action, and resumability, and must identify a prerequisite that repository-only Apply work cannot resolve. Category is never inferred from prose. Valid categories are `credential`, `external_approval`, `policy`, `external_service`, `pending_verification`, `infrastructure`, `schema_incompatibility`, and `human_decision`.

Persist validated Acceptance stalls outside the worktree under Conflux's XDG state area. Bind each record to repository identity, change ID, managed worktree identity/path, branch when available, Apply revision, stalled phase, retry count, blocker evidence, resumability, next action, and timestamps. Use atomic writes and a versioned schema. The record controls only dispatch suppression, operator-facing stalled status, explicit retry phase, and blocker/retry presentation.

On restart and explicit retry, reconcile the record against current Git and workspace facts. A valid resumable record preserves `stalled`; explicit retry consumes it transactionally with dispatch preparation and resumes at Acceptance without rerunning Apply. A stale, corrupt, missing, path-reused, revision-mismatched, archived, or merged record cannot override repository evidence. When runtime state is absent and repository evidence proves a complete unarchived Apply commit, run Acceptance again and never infer PASS.

Stop creating Acceptance-origin `APPLY_BLOCKED` markers. Migrate legacy structured Acceptance-origin markers to validated runtime state only when their identity, revision, and blocker payload can be established safely; remove the generated marker without leaving a dirty worktree. Apply-origin, unknown-origin, non-resumable, and malformed markers retain their existing conservative handling and are never silently migrated or deleted. Removing the Apply marker contract entirely is not part of this change.

Keep this as one hybrid proposal because the Constitution exception, persisted state boundary, verdict schema, restart routing, explicit retry, marker migration, and prompt contract must ship together. Splitting them would temporarily leave either unconstitutional routing or an unrecoverable stalled state.

## Acceptance Criteria

- Bare JSON `gated`, plain-text `ACCEPTANCE: GATED`, and legacy bare `blocked` create no file under the change directory and no durable stalled record.
- Bare blocker compatibility input is classified as an Acceptance protocol error, retried through Acceptance at most twice, never reruns Apply, and becomes a terminal protocol error after exhaustion.
- Bare input emits no `AcceptanceGated`/stalled lifecycle transition and receives no inferred credential, infrastructure, or other blocker category.
- Only a structured, validated, repository-external blocker enters `stalled`; invalid or incomplete blocker payloads follow the bounded protocol-error path.
- A validated Acceptance stall is atomically stored outside the worktree with repository, change, worktree, branch, Apply revision, phase, retry, blocker, resumability, next-action, schema, and timestamp fields.
- Entering, restarting, displaying, and explicitly retrying an Acceptance stall leave the managed worktree clean and preserve the Apply commit.
- Restart restores `stalled` only when runtime state matches the current repository, managed worktree, change, and Apply revision.
- Missing, stale, corrupt, mismatched, archived, or merged runtime state cannot prove PASS or override workspace/Git evidence; a complete unarchived Apply revision reruns Acceptance.
- Explicit retry of a valid resumable stall resumes at Acceptance without rerunning Apply and does not lose blocker evidence when dispatch preparation fails.
- Ordinary queue reconciliation does not re-submit a runtime-stalled change as `Blocked -> Blocked`; exhausted protocol errors require explicit retry.
- Legacy Acceptance-origin markers migrate safely when their ownership and binding can be validated, while Apply-origin, unknown-origin, non-resumable, and malformed markers are preserved conservatively.
- Acceptance runtime state does not participate in worktree dirty checks, Apply completion evidence, Acceptance PASS evidence, cleanup, archive readiness, merge eligibility, or canonical OpenSpec artifacts.
- Serial and parallel modes use the same blocker validation, protocol retry, persistence, reconciliation, migration, and retry decisions while serial mode remains supported.
- `openspec/CONSTITUTION.md` permits narrowly scoped, revision-bound runtime pause/resume state while retaining repository-verifiable truthful completion and fail-safe Acceptance reruns.

## Explicit Completion Conditions

- The Acceptance result model preserves structured blocker evidence and differentiates validated stalls from bare compatibility verdicts.
- Shared mode-independent logic validates blocker payloads, drives the bounded bare-GATED retry, and never applies prose-based category inference to bare input.
- A versioned XDG state repository implements atomic create/read/reconcile/consume/quarantine behavior and is covered without real external credentials or services.
- Serial and parallel orchestration use the shared state repository and decision logic; Acceptance-origin marker writes and marker-based restart/explicit-retry routing are removed from active paths.
- Workspace detection still handles Apply-origin and unknown `APPLY_BLOCKED` markers conservatively, but Acceptance stalls no longer precede Apply commit detection through generated repository files.
- Legacy migration has fixture-backed success, refusal, idempotency, and clean-worktree regressions.
- Prompt and embedded `cflx-accept` guidance require structured blocker evidence and explain that bare GATED is a protocol error rather than a stalled hold.
- Unit tests cover verdict parsing, blocker validation, category preservation, retry/reset/exhaustion, state schema, revision binding, and stale-state handling.
- Integration tests cover serial/parallel clean-worktree stalls, restart reconciliation, Acceptance-only explicit retry, failed retry preparation, queue non-reinsertion, runtime-state loss, and legacy migration.
- `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, and `cflx openspec validate replace-acceptance-marker-stalls --strict --evidence warn` pass.

## Out of Scope

- Removing or redesigning Apply-origin `APPLY_BLOCKED` handoffs.
- Persisting Acceptance PASS or using runtime state as implementation/archive/merge evidence.
- Resuming provider-specific agent sessions.
- Polling external jobs from the runtime based on untrusted narrative output.
- Making the retry limit configurable.
- Changing dependency-blocked lifecycle taxonomy.
