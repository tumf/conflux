## Context

Conflux parallel execution creates one worktree per change, applies and accepts changes there, then serializes archive integration and merge/resolve work through the project base lane. Existing pre-sync updates a change worktree from the current local base, but the cumulative local base is not refreshed from a remote branch during `cflx run`.

A long-running cumulative base is therefore a second integration stream alongside human/team updates to `origin/<base>`. Reconciliation must remain optional because automatic fetch, merge, credential use, and repository verification would change established CLI behavior. When explicitly enabled, it must remain Conflux-owned and recover from repository state alone.

## Goals / Non-Goals

### Goals

- Preserve current behavior unless run mode receives `-u` or `--integrate-upstream`.
- Integrate selected remote-base advances inside the cumulative parallel orchestration loop and push the verified cumulative result at successful completion.
- Keep native fetch, merge, verification, and push execution outside the AI command harness.
- Keep one workspace writer and one project base-lane owner.
- Let independent change-worktree apply/acceptance continue while blocking their base integration during the checkpoint.
- Delegate only textual or semantic repair to the configured `resolve_command` agent.
- Make interruption and retry derivable from Git/workspace evidence.
- Prevent stale cumulative-base push without force push.

### Non-Goals

- Persistent-config enablement or default-on behavior.
- TUI, no-subcommand TUI, serial, or server-mode integration.
- Replacing existing per-change pre-sync or `PushToRemote`.
- Rebase or cumulative-history rewriting.
- External workflow checkpoint storage or distributed locking.

## CLI Contract

```text
cflx run --all --parallel -u --upstream-verify-command "cargo test"
cflx run --all --parallel --integrate-upstream --upstream-verify-command "cargo test"
cflx run --all --parallel --integrate-upstream=upstream --upstream-verify-command "cargo test"
```

`-u` and value-less `--integrate-upstream` are flags selecting `origin`; neither consumes a following positional change ID. A named remote is accepted only as `--integrate-upstream=<remote>` with `=` required. The remote branch is the selected remote's branch with the same name as the checked-out cumulative base branch.

The option is stored as invocation-scoped runtime configuration and propagated from `RunArgs` through `main`, `Orchestrator`, `ParallelRunService`, and `ParallelExecutor`. It is not added to persistent orchestration config in this change.

Startup validation rejects before workspace mutation:

- non-parallel effective execution;
- detached HEAD;
- missing selected remote configuration before network access or missing same-name remote branch after the initial fetch;
- any commit reachable from starting local base HEAD but not from the initial-fetch remote SHA unless every such commit is a Conflux upstream merge carrying a valid `Cflx-Upstream-Merge: <fetched-sha>` trailer;
- missing or empty `--upstream-verify-command`;
- combination with `--push`, because that path pushes individual completed change branches instead of maintaining the cumulative base;
- non-Git VCS resolution.

Dry-run validates static option compatibility, selected remote configuration, non-empty verification command, repository type, attached HEAD, and local base cleanliness. It performs no network fetch or workspace mutation, so remote branch existence and initial-fetch ancestry are intentionally deferred to a real run.

## Decisions

### Decision: default-off behavior is a hard compatibility boundary

When the option is absent, no upstream checkpoint object is installed and no new fetch, merge, verification command, lifecycle event, or pre-push ancestry guard is added. This avoids hidden credential requirements and preserves current run behavior.

### Decision: Conflux owns normal Git integration

Conflux itself performs fetch, revision resolution, ancestry classification, and `git merge --no-ff <fetched-sha>`. An AI agent is not invoked for an ordinary conflict-free integration.

The top-level existing `resolve_command` is invoked only when:

1. repository state proves a textual conflict or unfinished merge requiring repair; or
2. the full verification command fails and the bounded semantic-repair policy permits another repair attempt.

Conflux owns retry limits and decides whether the repair converged. Agent narrative output never establishes success.

### Decision: synchronization exclusively owns only the base lane

The checkpoint starts only when the cumulative base is clean and the project base lane has no active archive integration, merge, resolve, or rejection review. It pauses new base integrations and dispatch decisions that require changed base evidence.

Apply and acceptance commands already running in independent change worktrees may continue. Their successful results remain queued and merge into the cumulative base after the checkpoint releases the lane; they are not discarded. Stale completion events cannot release checkpoint ownership.

### Decision: every upstream change uses a non-fast-forward merge

A fetched revision already contained in local base is a no-op. Any fetched revision not contained in local base, including a strictly remote-ahead revision, is integrated with `git merge --no-ff <fetched-sha>`.

The merge commit provides repository-visible evidence that an upstream checkpoint changed cumulative history and MUST include the trailer `Cflx-Upstream-Merge: <fetched-sha>`. Restart identification, fetched-SHA recovery, and ancestry validation use this trailer plus Git ancestry; a trailer-less merge is never classified as a Conflux upstream merge. Rebase, fast-forward integration, reset, amend, and force push are prohibited on the new path.

### Decision: merge classification comes from repository state

Human-readable Git output is not authoritative. Conflux classifies the result using command exit status, `MERGE_HEAD`, and unmerged index entries:

- success with no unfinished merge proceeds to reverification;
- `MERGE_HEAD` plus unmerged entries enters upstream repair;
- other non-zero outcomes are hard command failures and do not invoke the repair agent merely because output text resembles a conflict.

### Decision: upstream repair reuses bounded resolve machinery

The existing `resolve_command` runner, command queue policy, streaming, and retry budget are reused. It runs in the cumulative base checkout while preserving merge-in-progress state. The prompt identifies:

- local cumulative revision before integration;
- fetched remote SHA, selected remote, and base branch;
- unmerged files and Git status;
- whether the cause is textual conflict or semantic verification failure;
- the requirement to preserve accepted local and upstream intent;
- the prohibition on history rewriting.

Success requires no unmerged entries, no unfinished merge, and the fetched SHA as an ancestor of resulting local HEAD. Semantic repair may create forward commits only.

### Decision: explicit full verification is mandatory after tree change

`--upstream-verify-command` supplies the complete repository gate and runs from the cumulative base root after every actual upstream tree change. It runs after conflict-free and agent-repaired merges. In addition, every opted-in run executes the complete command unconditionally against final cumulative HEAD immediately before push, including ancestry-proven upstream no-op runs. Disabled and dry-run paths execute none.

A non-zero result blocks dispatch requiring new base evidence, base integration, and cumulative-base push. When repair is allowed, `resolve_command` may attempt a bounded semantic repair, followed by a mandatory rerun of the complete command.

### Decision: restart deliberately reruns verification

Ordinary command success does not create Constitution-compatible durable evidence. Therefore, after process restart, any valid trailer-identified upstream merge commit reachable from cumulative HEAD but not yet incorporated into the selected remote base is treated as unverified and executes the newly supplied full verification command again.

If such evidence exists and cumulative parallel run is invoked without `-u`, startup refuses to continue and requires the operator to select the same remote and supply a verification command. The prior command value is not recovered from external state. Runtime events, logs, TUI projections, or external journals cannot suppress this rerun.

### Decision: `-u` owns one final native cumulative-base push

A successful opted-in run is incomplete until Conflux pushes verified cumulative HEAD to the selected remote's same-name base branch. No second push option is required. After the initial fetch, startup rejects local-only commits not identifiable as Conflux upstream recovery commits; valid trailer-identified Conflux commits enter restart recovery instead.

Fetch, merge, verification command execution, and push are native Conflux operations and do not run through `AgentRunner`, `AiCommandRunner`, or an AI command template. Immediately before final push, Conflux fetches again and checks that the latest selected remote SHA is an ancestor of cumulative HEAD. If not, push is suppressed and integration restarts.

The actual push remains non-force and uses `git push --porcelain`. Classification uses only machine-readable per-ref status and post-failure `git status --porcelain=v2`:

- per-ref non-fast-forward/fetch-first/stale-info status is a race and returns directly to fetch/integration;
- tracked worktree mutation or unmerged entries after failure is repository-repairable and MAY invoke bounded `resolve_command` with sanitized diagnostics;
- every other failure stalls without an agent; stderr text is never routing input;
- the agent MUST NOT execute `git push`, alter credentials, bypass hooks, or claim push success;
- after agent repair, Conflux MUST recheck repository convergence, rerun full verification, fresh-fetch, and execute the non-force push itself.

Race handling may produce multiple failed push attempts, but a run may record at most one successful push and must never retry after confirmed success. Confirmation uses `git ls-remote` network observation: completion requires pushed HEAD equal to the observed remote SHA, or after a further remote advance, pushed HEAD to be an ancestor of the freshly fetched observed SHA. Otherwise control returns to fetching.

## State Flow

```text
OPTION ABSENT
  -> EXISTING RUN PATH

OPTION ENABLED / BASE-LANE SAFE POINT
  -> FETCHING_UPSTREAM
     -> fetched SHA already ancestor -> RUNNING
     -> fetched SHA not ancestor -> INTEGRATING_UPSTREAM (--no-ff)

INTEGRATING_UPSTREAM
  -> complete merge -> REVERIFYING
  -> MERGE_HEAD + unmerged entries -> RESOLVING_UPSTREAM
  -> other command failure -> STALLED

RESOLVING_UPSTREAM
  -> converged repository -> REVERIFYING
  -> retry exhausted -> STALLED

REVERIFYING
  -> pass -> RUNNING
  -> repairable failure -> RESOLVING_UPSTREAM
  -> retry exhausted -> STALLED

READY_TO_PUSH
  -> fresh fetch; latest remote ancestor -> NATIVE_NON_FORCE_PUSH
  -> remote advanced -> FETCHING_UPSTREAM

NATIVE_NON_FORCE_PUSH
  -> success + remote confirmation -> COMPLETED
  -> race-time rejection -> FETCHING_UPSTREAM
  -> repository-repairable failure -> RESOLVING_UPSTREAM
  -> credential/transport/policy failure -> STALLED
```

## Failure and Recovery

- Unsafe base-lane state defers the entire checkpoint, including fetch.
- Fetch failure leaves the worktree untouched and fails/stalls according to bounded command policy.
- A crash during merge is detected from `MERGE_HEAD` and unmerged entries.
- A crash after merge reruns full verification because prior command success is not repository-provable.
- A crash after verification performs verification again when the upstream merge remains unpushed, then fresh-fetches before push.
- Explicit retry re-evaluates repository facts; no in-memory state is trusted to skip work.

## Risks / Trade-offs

- Non-fast-forward merge creates an extra commit even for strictly remote-ahead history. This is intentional crash-recovery evidence and keeps behavior deterministic.
- Full reverification may be expensive. The feature is opt-in and favors correctness; tiered verification is future work.
- A remote that advances continuously may starve base integration. Batching and debounce are future work, not reasons to weaken the final check.
- The verification command is user-provided shell execution. It is explicit CLI input, runs only when the feature is enabled, and uses existing command execution safety and logging conventions.
