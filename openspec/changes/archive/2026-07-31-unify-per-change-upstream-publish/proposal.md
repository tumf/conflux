---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/upstream-integration/spec.md
  - openspec/specs/orchestration-state/spec.md
  - openspec/specs/cli/spec.md
  - src/cli.rs
  - src/main.rs
  - src/tui/runner.rs
  - src/tui/orchestrator.rs
  - src/parallel/merge.rs
  - src/parallel/upstream_lane.rs
  - src/upstream/coordinator.rs
  - src/orchestration/state.rs
  - src/events.rs
verifications:
  - id: per-change-upstream-unit
    requirement: Shared run/TUI option parsing, per-change publication ordering, durable recovery evidence, lifecycle transitions, explicit TUI retry routing, option-less restart refusal, and default-off compatibility are covered by repository-local tests.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/mod.rs
    evidence: cargo test output for per_change_upstream and upstream_integration unit cases
    rerun: cargo test per_change_upstream && cargo test upstream_integration
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: per-change-upstream-e2e
    requirement: Real local Git repositories and bare remotes prove that run and local TUI use the same change-scoped fetch, verification, native push, remote-confirmation, failure, and recovery path.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/e2e_git_worktree_tests.rs
    evidence: heavy-tests output for per_change_upstream cases
    rerun: cargo test --features heavy-tests --test e2e_git_worktree_tests per_change_upstream
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Unify per-change upstream publication across run and TUI

**Change Type**: implementation

## Problem / Context

Opt-in upstream integration currently belongs only to cumulative parallel `cflx run`. It reconciles the selected remote during the run, but publishes cumulative base once after the scheduler drains. Local TUI cannot enable the capability because `TuiArgs` and the TUI orchestration path do not carry upstream runtime configuration.

That creates two mismatches with the desired workflow:

- `run` treats upstream publication as a run-level finalization concern even though each OpenSpec change reaches a repository-visible completion boundary independently;
- local TUI cannot use the same safe upstream integration path, so its terminal success remains `merged` even when the operator wants remote publication to be part of completion.

The existing reducer already has `TerminalState::Pushed` and `ExecutionEvent::PushCompleted`, but the cumulative upstream path does not attribute confirmed publication to each completed change. The existing upstream coordinator also records at most one successful push per invocation, which cannot represent a long-lived TUI publishing multiple changes.

## Proposed Solution

Make `-u` / `--integrate-upstream` one invocation-scoped cumulative-parallel capability with the same change-level semantics in non-interactive run and local TUI.

Supported invocations are:

```text
cflx run --all -u --upstream-verify-command '<command>'
cflx run <change-id>... -u --upstream-verify-command '<command>'
cflx -u --upstream-verify-command '<command>'
cflx tui -u --upstream-verify-command '<command>'
cflx tui --integrate-upstream=<remote> --upstream-verify-command '<command>'
```

Value-less `-u` and `--integrate-upstream` select `origin`; a named remote remains accepted only as `--integrate-upstream=<remote>`. Bare and explicit TUI invocations MUST validate and construct the same upstream runtime configuration as `run`. Remote-client TUI (`tui --server`), server orchestration, serial execution, per-change pre-sync, and `PushToRemote` remain outside this capability. `-u` remains incompatible with `--push`.

When `-u` is enabled, every change that passes acceptance, archives, and integrates into cumulative base MUST complete this shared base-lane sequence before another completed change may integrate:

1. reconcile the selected remote's same-name base branch at the existing pre-result safe point;
2. merge the archived change into cumulative base;
3. execute `on_merged` successfully;
4. run the complete upstream verification command against cumulative base;
5. fresh-fetch and integrate any remote advance, then reverify as required;
6. execute the existing native non-force porcelain push;
7. confirm through `git ls-remote` that the published cumulative HEAD is reachable from the remote branch;
8. emit change-scoped push completion and set that change's terminal state to `pushed`.

The base lane remains held through local integration, verification, publication, and remote confirmation. Independent apply and acceptance work in other worktrees may continue, but later archived results wait before cumulative-base integration. This preserves deterministic attribution: each `pushed` change identifies a confirmed cumulative HEAD that contains that change.

Without `-u`, existing behavior is unchanged and successful cumulative base integration terminates the change as `merged`. With `-u`, local merge is repository progress rather than terminal success; `pushed` is the only successful terminal state for that change. User-facing `merged` MUST NOT be reported as the final outcome of an opted-in change.

A failed fetch, merge, verification, push, or remote confirmation MUST leave the change in a visible resumable upstream-publication wait/error state, MUST NOT emit `PushCompleted`, and MUST NOT redispatch apply or acceptance. In persistent local TUI, exhaustion of the bounded publication cycle MUST enter the existing operator-visible recoverable error flow; F5 or equivalent local web-control retry MUST resume publication, retain base-lane ownership while pending, and release the lane for waiting results only after remote confirmation or operator stop. Explicit retry and restart MUST derive the next action from cumulative-base, archive, durable publication-intent trailers, and remote ancestry evidence, resume the unpublished change at the upstream publication boundary, and preserve the existing prohibition on force-push, rebase, reset, and amend.

Before an opted-in archived result can be treated as locally integrated, its cumulative-base integration commit MUST record repository-visible Git identity that binds the change, selected remote, and base branch to required publication. Remote reachability of that marked integration determines whether publication is complete after process loss. An option-less restart that finds a marked integration not proven reachable from its bound remote branch MUST fail before orchestration mutation and MUST NOT reclassify the change as terminal `merged`; recovery requires `-u` and a fresh verification command.

The run frontend exits successfully only after every targeted successful change is `pushed`. The local TUI remains active after each change becomes `pushed` and may accept more queue work; each later completion starts another publication cycle through the same shared service. `AllCompleted` for an opted-in finite run is emitted only after all targeted changes are remotely confirmed. A persistent TUI does not require scheduler drain to publish a completed change.

Fresh zero-change invocations preserve current no-work behavior. Zero-change recovery recognizes only repository-visible integration evidence that explicitly records required upstream publication; it may verify and publish that cumulative history without manufacturing a synthetic change terminal event. A change that previously reached terminal `merged` without opted-in publication is not retroactively promoted to `pushed`, although a later cumulative publication may naturally contain its commits.

## Acceptance Criteria

1. `cflx run -u`, bare local `cflx -u`, and `cflx tui -u` construct the same selected-remote and verification-command configuration and execute the same shared per-change publication service.
2. Value-less upstream options select `origin`; explicit remotes require `--integrate-upstream=<remote>`; missing verification commands and incompatible `--push`, serial, remote-client TUI, server, detached-HEAD, non-Git, or invalid remote/base conditions fail before orchestration mutation.
3. With `-u`, each accepted and archived change is integrated, verified, natively pushed, and remotely confirmed before that change reaches successful terminal state.
4. With `-u`, successful change terminal state and display status are `pushed`, not `merged`; `PushCompleted` is emitted only after `git ls-remote` confirmation.
5. Without `-u`, cumulative parallel changes retain the existing `merged` terminal state and no new fetch, verification, push, or upstream event occurs.
6. Multiple completed changes may apply and accept concurrently, but their base integration and publication cycles are serialized; a failed or stalled prior publication keeps later results before base integration until explicit retry confirms it or the operator stops orchestration.
7. A persistent TUI publishes each completed change without waiting for scheduler drain, remains usable after publication, exposes exhausted publication failure through its existing F5/local-web explicit retry flow, and can publish later queued changes through fresh publication cycles.
8. A finite run emits `AllCompleted` and exits successfully only after every targeted successful change reaches `pushed`; blocked, stalled, failed, or cancelled publication never reports completion.
9. An opted-in local integration records durable Git evidence binding change, remote, and branch before it may become publication-pending; option-less restart refuses marked unpublished history instead of reporting terminal `merged`.
10. A publication failure remains visible and resumable, does not regress to ordinary apply work, and retry/restart resumes from repository and remote evidence without duplicate local integration or duplicate confirmed success; in-memory confirmed-HEAD state is only an optimization after remote-observed confirmation.
11. Zero-change recovery recognizes only explicit opted-in publication evidence, and disabled-mode terminal `merged` changes are not retroactively promoted to `pushed`.
12. Remote races return to bounded fetch/integration/reverification; repository-repairable failures use the existing bounded repair path; credential, permission, transport, hook-policy, and remote-service failures stall without agent speculation.
13. Every confirmed publication remains native, non-force, and Conflux-owned; an agent never pushes or establishes remote success.
14. Unit and real-Git E2E tests prove run/TUI parity, per-change push count and ordering, durable crash recovery identity, option-less restart refusal, terminal-state behavior, default-off compatibility, operator-visible TUI retry, lane blocking/release, failure suppression, idempotent recovery, and remote confirmation.

## Explicit Completion Conditions

- CLI parsing and startup validation cover top-level TUI, explicit `tui`, and `run` upstream options with one normalized runtime configuration and no support in remote-client TUI or server mode.
- TUI startup passes optional upstream configuration into the same parallel execution builder/service used by `run`; no TUI-specific fetch, verification, push, or confirmation implementation exists.
- The upstream coordinator exposes a reusable change-scoped publication operation that can confirm more than one successive cumulative HEAD per process while remaining idempotent for an already remote-confirmed HEAD; restart and ambiguous outcomes always re-observe remote ancestry.
- Post-archive base-lane handling records durable publication identity before treating an opted-in result as locally integrated, does not finalize it at local merge, and runs `on_merged`, verification, fresh remote reconciliation, native push, and remote confirmation before emitting change-scoped successful completion.
- Reducer and frontend projections represent opted-in confirmed publication as `pushed`, prevent a local or recovered `merged` observation from becoming the final opted-in outcome, preserve `merged` for disabled mode, and route exhausted TUI publication failure through existing explicit retry controls.
- Failure and retry wiring preserves unpublished repository evidence, exposes a resumable state, prevents ordinary apply redispatch, keeps later base integration blocked, and resumes publication without rewriting history or duplicating confirmed success.
- Startup and recovery tests prove option-less restart refuses marked unpublished integration, enabled restart resumes it with fresh verification, zero-change recovery requires explicit opted-in evidence, and disabled-mode `merged` is not retroactively promoted.
- Scheduler tests prove other worktrees may continue apply/acceptance while base integration waits, only one publication owns the base lane, TUI F5/local-web retry resumes publication, lane release follows confirmation or operator stop, and TUI publication does not depend on finite drain.
- Real-Git E2E tests use local bare remotes to prove durable opted-in integration identity, one and multiple change publications, remote advance/race handling, failed verification and push suppression, option-less refusal, enabled restart recovery, run completion ordering, and repeated TUI publication cycles.
- `cargo test per_change_upstream`, `cargo test upstream_integration`, `cargo test --features heavy-tests --test e2e_git_worktree_tests per_change_upstream`, `cargo fmt --check`, and `cargo clippy --all-targets --all-features -- -D warnings` pass.

## Out of Scope

- Enabling upstream integration by default or through persistent project configuration.
- Supporting serial execution, remote-client TUI, server orchestration, server `git-sync`, or distributed multi-process publication.
- Changing per-change pre-sync or `PushToRemote` branch-push semantics.
- Adding a second terminal label such as `synced`; the existing `pushed` state is authoritative.
- Rebase, force-push, reset, amend, or other cumulative-history rewriting.
- Inferring repository verification commands instead of requiring explicit CLI input.
