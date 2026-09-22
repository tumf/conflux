---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/proposal-metadata/spec.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - src/orchestration/acceptance/verification_evidence.rs
  - src/parallel/executor.rs
  - src/parallel/dispatch.rs
  - src/web/remote_control_api/dto.rs
  - skills/cflx-accept/SKILL.md
verifications:
  - id: bounded-acceptance-runtime-tests
    requirement: Acceptance executes and reuses only declared change-blocking verification, terminates with a typed outcome before its absolute deadline, and refuses unchanged blind retry after boundary exhaustion.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/executor.rs
    evidence: cargo test acceptance_execution_boundary --lib
    rerun: cargo test acceptance_execution_boundary --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Bind Acceptance execution to declared verification

**Change Type**: implementation

## Problem / Context

Proposal `verifications` currently influences only an advisory reuse block in the Acceptance prompt. `src/parallel/executor.rs` explicitly treats the plan as observation rather than control, and `skills/cflx-accept/SKILL.md` still asks a reviewer to perform broad regression review and wait for every verification it starts. An Acceptance agent can therefore expand from one focused, passing change gate into repository-wide suites, base-worktree comparisons, or unrelated artifact investigation.

The absolute runtime limit terminates that unbounded review as an execution error. Repository-local gate success is not enough to recover: the timeout records no canonical Acceptance attempt, and `retry_change` accepts the same workspace input without a verification boundary or unchanged-input guard, so it dispatches the same exploration again.

This was reproduced against candidate `541642b837f0a22175b55def96b1ba2c44e56356`, review range `73f2207a2af656c8d1ad74b820225a631ca04966..541642b837f0a22175b55def96b1ba2c44e56356`. Its sole declared `change-blocking` verification was `sync-exclusion-release-focused-gate`; that gate and the repository commit checks passed, while two Acceptance invocations expanded outside the declaration and reached `acceptance_max_runtime_secs=3600`.

## Proposed Solution

Create one runtime-owned Acceptance execution manifest from workspace evidence before dispatch. It binds the candidate commit/tree, review base and range, canonicalized change inputs, the selected Acceptance skill identity, and every eligible `pre-integration` / `repository-local` / `change-blocking` verification. Its SHA-256 digest is the Acceptance input fingerprint.

Conflux, not the reviewer, wires the existing CLI-only `RuntimeVerificationExecutor` into Acceptance, executes missing declared gates, and reuses exact bound evidence when valid. The resulting gate set is the complete blocking command allowlist. The semantic reviewer receives the frozen manifest, diff, proposal/tasks/specs, and gate results. It may inspect in-range repository content read-only, but it must not start repository-wide/full-suite/base-worktree or undeclared verification work. Any already-complete observation may inform a finding, but its absence, failure, or incompletion cannot delay or block the verdict.

Declared gates and semantic review share the existing effective Acceptance deadline. Reserve its final 30 seconds for termination, reap, persistence, and state projection. An external prerequisite retains the existing resumable external stalled-hold contract. Gate/review deadline exhaustion, malformed manifest, runtime defect, or unproven cleanup becomes a non-resumable `stalled` execution hold with a stable category; it never becomes a generic runtime-limit `error`. Review-deadline termination bypasses missing-verdict retry.

Persist the runtime-owned manifest, gate outcomes, and terminal reason in the existing Git-excluded repository evidence area. Exact fingerprints reuse successful declared gate results. A retry with the same fingerprint after a deadline hold is refused as `unchanged_acceptance_input`; it does not dispatch another Acceptance agent. The sanctioned recovery is a repository-visible change to candidate revision/tree, review base/range, canonical change input, embedded skill/executable identity, declaration, automation blob, command/tool identity, or bound artifact digest. Such a change creates a new fingerprint and permits one fresh bounded attempt.

## Acceptance Criteria

- One eligible declared change-blocking verification yields one blocking command allowlist; undeclared repository-wide, full-suite, base-worktree, sibling-change, and unrelated artifact checks are never required for PASS and never keep Acceptance open.
- Existing runtime-authored evidence is reused only when its candidate revision/tree, review base/range, normalized declaration and command, automation blob, tool identity, artifact digest, and clean-state bindings match the current execution manifest.
- The semantic reviewer receives the candidate revision, exact review range, input fingerprint, declared gate outcomes, and an explicit read-only review boundary.
- Every Acceptance invocation settles as `pass`, `fail`, an existing resumable external hold, or a non-resumable `stalled` execution hold before the existing absolute runtime limit. Process-group cleanup remains proven and no descendant survives.
- Runtime-limit expiry remains a safety fallback for command-runner defects, but ordinary bounded Acceptance does not expose it as the change outcome.
- `retry_change`, bulk retry, Start/F5, terminal-error queue aliases, and mark settlement refuse an unchanged deadline-hold fingerprint without dispatching analysis, Apply, gates, or Acceptance. The response names `unchanged_acceptance_input` and the fingerprint components whose repository-visible change restores eligibility.
- A changed fingerprint permits one new bounded attempt and reuses every still-valid declared gate result instead of repeating it.
- All workflow-control authority remains derivable from workspace file state, workspace Git state, and base-tree comparison. No global setting, project-specific `.cflx.jsonc` workaround, external cache, or hidden durable state is introduced.

## Explicit Completion Conditions

- Runtime code constructs, hashes, atomically persists, parses, and validates the versioned Acceptance execution manifest from repository-local evidence.
- The declared verification executor and reuse validator feed typed gate outcomes into Acceptance dispatch; prompt-only advisory reuse is no longer the owner of whether a blocking gate runs.
- The portable `cflx-accept` skill and runtime adapter define the same closed blocking boundary and forbid reviewer-owned expansion from delaying a verdict.
- Executor/dispatch tests use a fake reviewer that emits a canonical verdict without requesting extra work and a non-terminating reviewer. They prove that only runtime-owned declared gates execute, finalization occurs 30 seconds before the absolute deadline, cleanup is confirmed, and the outcome is `stalled` rather than generic `error`.
- Run-control/API tests prove unchanged retry refusal and changed-fingerprint retry admission across `retry_change`, bulk retry, Start/F5 retry, and terminal-error queue aliases.
- Restart tests delete out-of-worktree Conflux state and derive the same manifest, reuse decision, retry eligibility, and next action from the preserved workspace alone.
- `cargo test acceptance_execution_boundary --lib` passes.

## Out of Scope

- Changing per-project `.cflx.jsonc`, shared user skills, or global runtime limits to accommodate one change.
- Treating a proposal declaration as proof that implementation semantics are correct; semantic diff review remains required.
- Reimplementing the reviewed product, manually forcing PASS, or merging around Acceptance.
- Trusting agent-authored evidence or external CI state as blocking authority.
- General-purpose shell sandboxing for arbitrary third-party agent CLIs. Conflux instead owns blocking verification execution and refuses to wait on reviewer-started undeclared work.

## Verification Ownership

The focused Rust regression target owns manifest identity, declared-gate execution/reuse, deadline finalization, cleanup, and unchanged-retry behavior. Repository-wide checks remain owned by the tracked commit hook and are not part of this change's Acceptance allowlist.

## Final Validation

Archive validation is authoritative: `cflx openspec validate bound-acceptance-verification --archive-gate`.
