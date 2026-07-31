## Context

The current upstream integration path already owns native fetch, non-fast-forward merge, complete verification, bounded repair, non-force porcelain push, and `ls-remote` confirmation. Its lifecycle is finite-run scoped: the coordinator stores one confirmed `pushed_head`, finalization runs only after `DrainedSuccessfully`, and only `RunArgs` carries the option.

The local TUI uses the same parallel executor for change work but keeps a persistent scheduler lifetime. Waiting for drain therefore cannot publish a completed TUI change, and constructing a separate TUI push path would duplicate the safety-critical Git protocol. The reducer already understands `TerminalState::Pushed`; the missing contract is to attribute confirmed cumulative-base publication to the change that just integrated.

## Goals / Non-Goals

### Goals

- Give `-u` identical change-level meaning in non-interactive run and local TUI.
- Publish each accepted, archived, cumulative-base-integrated change before declaring its success.
- Use `pushed` as the opted-in successful terminal state and retain `merged` when disabled.
- Keep one Conflux-owned base lane through merge, verification, push, and remote confirmation.
- Permit other worktrees to continue independent apply/acceptance while later base integration waits.
- Preserve all existing native-operation, bounded-repair, non-force, and repository-evidence safety rules.
- Make opted-in publication intent and incomplete publication derivable after process loss from Git evidence.
- Support multiple successive publication cycles in one persistent TUI process.

### Non-Goals

- Default-on or persistent-config enablement.
- Remote-client TUI, server orchestration, or serial mode.
- A second `synced` terminal state.
- Replacing per-change pre-sync or `PushToRemote`.
- Batching multiple completed changes into one publication.

## Decisions

### Decision: one capability, one shared service

`run`, bare local TUI, and explicit local `tui` normalize CLI inputs into one optional `UpstreamIntegrationConfig`. The TUI passes that configuration into the existing parallel service/builder. Frontends may differ in lifetime and presentation, but they do not implement fetch, verification, push, confirmation, or recovery.

Remote-client TUI has no local repository ownership and cannot safely execute this protocol. It rejects `-u` before orchestration. Server support remains separate because a remote control contract would need to carry the selected remote, verification command, credentials boundary, and lifecycle evidence explicitly.

### Decision: publication belongs to the completed change boundary

The atomic base-lane operation becomes:

```text
PRE-RESULT UPSTREAM CHECKPOINT
  -> MERGE ARCHIVED CHANGE INTO CUMULATIVE BASE
  -> ON_MERGED HOOK
  -> COMPLETE VERIFICATION
  -> FRESH UPSTREAM CHECKPOINT IF REMOTE ADVANCED
  -> COMPLETE REVERIFICATION WHEN TREE CHANGED
  -> NATIVE NON-FORCE PUSH
  -> LS-REMOTE CONFIRMATION
  -> CHANGE PUSHED
  -> RELEASE BASE LANE
```

Holding the lane through confirmation prevents a later change from entering cumulative base before the prior change's published revision is known. Each confirmed remote revision therefore contains the attributed change. Apply and acceptance in separate worktrees remain independent of this lane.

### Decision: `merged` is progress under `-u`, not success

Without upstream integration, `MergeCompleted` retains its existing terminal `Merged` meaning.

With upstream integration, local merge completion must not commit the reducer to a final state that rejects later `PushCompleted`. The shared execution path must either suppress the terminal `MergeCompleted` transition for that change or carry explicit publication-required context that keeps it non-terminal. Remote confirmation then emits change-scoped `PushCompleted`, producing `TerminalState::Pushed` and display status `pushed`.

A new `synced` state is unnecessary and would duplicate the existing final-status, retry-exclusion, TUI, WebSocket, and API taxonomy already built around `pushed`.

### Decision: publication intent is durable Git evidence

Before an opted-in archived result becomes publication-pending on cumulative base, the integration commit records recognizable trailers binding the change ID, selected remote, and same-name base branch to required publication. This marker is distinct from the existing fetched-SHA identity on upstream merge commits: it identifies why a locally integrated change must not be recovered as ordinary terminal `merged` when no remote advance produced an upstream merge.

Startup and retry compare the marked integration commit with the bound remote branch. A marked commit that is remote-reachable is confirmed publication evidence; one that is not proven remote-reachable remains unpublished. An option-less restart refuses the latter before orchestration mutation and directs the operator to restart with `-u` and a fresh verification command. This keeps routing derivable from Git and remote evidence instead of process memory.

Ordinary disabled-mode merge commits carry no publication-required marker. They retain terminal `merged`; later use of `-u` may publish a cumulative HEAD containing those commits but does not retroactively change their terminal state. Zero-change recovery recognizes only explicit publication-required markers or valid existing upstream recovery trailers, never arbitrary local first-parent history.

### Decision: coordinator idempotency is revision-scoped, not run-scoped

The current one-successful-push-per-run invariant becomes one-successful-push-per-cumulative-HEAD. The coordinator records the latest confirmed remote-reachable HEAD and may publish again only after cumulative HEAD advances.

A repeated request for the already confirmed HEAD is an idempotent no-op. The in-process record is only an optimization after that HEAD was confirmed by remote observation in the same process. Restart and any ambiguous push/confirmation outcome must re-run `ls-remote` and ancestry classification; the record is never workflow authority. A later change that advances cumulative HEAD starts a fresh bounded publication cycle. Failed attempts remain bounded per publication cycle and never authorize force-push or duplicate success events.

### Decision: finite run completion and persistent TUI lifetime are projections

A finite run finishes when every targeted change that completed successfully has reached `pushed`; only then may it emit `AllCompleted` and exit zero.

A local TUI publishes at the same change boundary but does not exit or require `DrainedSuccessfully`. After one change reaches `pushed`, the scheduler returns to waiting for queue input. A later queued change reuses the installed coordinator and starts a new revision-scoped publication cycle.

This removes final-drain push as the normal publication mechanism. A zero-change invocation still uses no-work/recovery finalization where there is no change to attribute.

### Decision: failure is resumable publication work

An opted-in change whose local integration exists but publication is incomplete must not become ordinary queued apply work. The runtime exposes a visible resumable publication wait/error and reserves the base lane from later integration until retry succeeds or the operator stops the run.

In persistent local TUI, exhausting the bounded publication cycle projects the owning change into the existing recoverable Error-mode interaction. F5 and the equivalent local web-control action invoke explicit retry for publication work, not apply or acceptance. The lane remains unavailable to later completed-result integration while this state is displayed; successful remote confirmation transitions the change to `pushed` and releases waiting integration. Operator stop ends orchestration without claiming publication success. No scheduler or time-based automatic retry is added.

Retry re-evaluates:

- archive evidence and whether the change is already integrated into base;
- current cumulative HEAD and clean/unfinished Git state;
- upstream identity trailers and fetched-SHA ancestry;
- latest selected remote branch revision;
- whether the current cumulative HEAD is already remote-reachable.

No event, log, or in-memory flag may substitute for this repository and network evidence. If a push likely succeeded but confirmation was interrupted, retry first observes the remote and emits success without pushing again when reachability is proven.

## State Flow

```text
OPTION ABSENT
  ARCHIVED -> RESOLVING -> MERGED (terminal)

OPTION ENABLED
  ARCHIVED
    -> RESOLVING
    -> LOCAL_BASE_INTEGRATED (non-terminal)
    -> VERIFYING
    -> PUBLISHING
       -> remote advance/race -> UPSTREAM_INTEGRATING -> VERIFYING
       -> repository-repairable -> RESOLVING_UPSTREAM -> VERIFYING
       -> non-repairable failure -> PUBLISH_WAIT/ERROR
    -> REMOTE_CONFIRMED
    -> PUSHED (terminal)

RETRY / RESTART
  -> inspect base/archive/trailers/remote
  -> already remote-reachable -> PUSHED
  -> unpublished integrated result -> VERIFYING -> PUBLISHING
  -> incomplete merge/repair evidence -> RESOLVING_UPSTREAM
```

## Failure and Recovery

- Verification failure keeps the base lane closed and emits no `PushCompleted`.
- Remote race returns to upstream integration and complete reverification under existing bounds.
- Credential, permission, transport, hook-policy, or remote-service failure stalls without agent invocation.
- Repository mutation or unfinished merge may use the existing bounded repair agent, but Conflux reruns convergence and verification and performs the push itself.
- Cancellation never marks an unpublished change successful.
- An opted-in local integration records its publication-required change/remote/branch identity in Git before process loss can leave it pending.
- Option-less restart refuses a marked integration not proven remote-reachable and never recovers it as terminal `merged`.
- Restart with a locally integrated unpublished change requires `-u` and a fresh verification command and resumes from repository evidence.
- Restart after an unconfirmed push checks remote reachability before attempting another push.
- A disabled-mode change already terminal `merged` is never retroactively promoted; zero-change recovery accepts only explicit opted-in evidence.

## Risks / Trade-offs

- Per-change verification and push cost more than one run-final publication. This is required by the requested completion contract; batching would make individual `pushed` attribution ambiguous.
- Holding the base lane through network operations delays later base integrations. Independent worktree work continues, and correctness takes priority over throughput on an explicit opt-in path.
- `on_merged` keeps its existing meaning of successful local base integration and runs before publication. Its failure prevents push and final success.
- Existing consumers that assume `MergeCompleted` is always terminal need explicit mode-aware behavior; tests must protect disabled-mode compatibility and enabled-mode promotion to `pushed`.
