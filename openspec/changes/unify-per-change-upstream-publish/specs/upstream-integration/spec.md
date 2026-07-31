## MODIFIED Requirements

### Requirement: Upstream integration is an opt-in run-mode capability

Conflux MUST preserve existing behavior unless cumulative parallel orchestration is explicitly invoked with `-u` or `--integrate-upstream`. This capability MUST be available with non-interactive `cflx run`, bare local TUI, and explicit local `cflx tui`, and all three entrypoints MUST normalize to the same invocation-scoped upstream runtime configuration and shared publication service.

The value-less option names MUST be exact aliases selecting remote `origin` and MUST NOT consume following positional change IDs. A named remote MUST be accepted only as `--integrate-upstream=<remote>` with `=` required; `-u <remote>` MUST NOT be supported.

Enabling upstream integration MUST require an explicit complete verification command and MUST make remote-confirmed cumulative-base publication part of every completed change's success contract. Conflux MUST reject unsupported or incomplete invocation combinations before mutating the workspace, including unrelated local-only first-parent integration history after a real invocation's initial fetch; valid cumulative change integrations with matching commit-tree archive evidence and valid upstream integration commits MUST enter recovery instead of being rejected. The option MUST NOT silently enable upstream integration in serial mode, remote-client TUI, server orchestration, per-change pre-sync, or `PushToRemote` workflows.

#### Scenario: option is absent

**Given**: a user starts run or local TUI without `-u` or `--integrate-upstream`
**When**: cumulative parallel orchestration executes
**Then**: Conflux follows the existing execution path
**And**: successful local base integration terminates each change as `merged`
**And**: it performs no new upstream fetch, cumulative upstream merge, upstream reverification, push, or upstream lifecycle event

#### Scenario: run and local TUI options are equivalent

**Given**: one invocation uses `cflx run -u`, one uses bare `cflx -u`, and one uses `cflx tui -u`
**When**: none supplies a remote value
**Then**: all produce identical enabled runtime configuration for remote `origin`
**And**: they use the same change-scoped upstream publication service
**And**: a named remote is accepted only as `--integrate-upstream=<remote>`
**And**: `-u` does not consume a following change ID as a remote value

#### Scenario: invalid invocation fails before mutation

**Given**: upstream integration is requested without cumulative parallel execution, with `--push`, from detached HEAD, for a missing remote or same-name remote branch, without a non-empty verification command, through remote-client TUI, or through server orchestration
**When**: Conflux validates startup
**Then**: it rejects the invocation before fetch, merge, resolve, verification, push, worktree creation, or orchestration mutation
**And**: the diagnostic identifies the invalid option, mode, or repository precondition

#### Scenario: dry-run suppresses side effects

**Given**: upstream integration options are valid for non-interactive run
**When**: the user requests parallel dry-run
**Then**: Conflux validates static option compatibility, selected remote configuration, attached HEAD, repository type, base cleanliness, and non-empty verification command
**And**: it defers remote branch existence and fetched ancestry validation to the real run's initial fetch
**And**: it performs no network fetch, merge, resolve command, verification command, or push

### Requirement: Running cumulative base integrates upstream changes at base-lane safe points

When opt-in upstream integration is enabled, cumulative parallel Conflux orchestration MUST refresh and reconcile the selected remote branch with the same name as the checked-out cumulative base branch at deterministic project base-lane checkpoints: before first worktree dispatch, immediately before each completed result enters base, immediately before publishing that result, after a fresh pre-push fetch observes remote advance, and after race-time non-fast-forward push rejection. A persistent local TUI MUST use these boundaries without waiting for scheduler drain. Conflux MUST remain the sole writer and MUST perform ordinary fetch, ancestry classification, merge, push, and confirmation itself; an external supervisor, frontend, or AI agent MUST NOT select or perform that ordinary integration workflow.

The safe point MUST require a clean cumulative base and exclusive base-lane ownership. Independent apply and acceptance commands in change worktrees MAY continue, but completed results MUST enter and publish from cumulative base one at a time. A later result MUST NOT enter cumulative base until the prior integrated result is remotely confirmed. A failed or explicitly stalled publication MUST keep the lane closed until explicit retry succeeds or the operator stops orchestration. Scheduler-loop polling and time-based polling MUST NOT be introduced. The authoritative routing decision MUST be derivable from workspace files, Git state, fetched refs, remote observation, and base-tree comparison.

#### Scenario: unchanged upstream is a checkpoint no-op

**Given**: opt-in upstream integration is enabled
**And**: the freshly fetched remote revision is already an ancestor of cumulative local HEAD
**When**: Conflux evaluates a pre-result checkpoint
**Then**: it records an ancestry-proven checkpoint no-op
**And**: it does not merge or invoke `resolve_command` for that checkpoint
**And**: the completed change may proceed into cumulative base integration and mandatory publication verification

#### Scenario: upstream advance uses a non-fast-forward merge

**Given**: Conflux owns a clean project base lane
**And**: the freshly fetched remote revision is not an ancestor of cumulative local HEAD
**When**: Conflux integrates the revision
**Then**: Conflux itself runs a non-fast-forward merge of the fetched SHA
**And**: the merge commit records `Cflx-Upstream-Remote: <remote>`, `Cflx-Upstream-Branch: <branch>`, and `Cflx-Upstream-SHA: <fetched-sha>` for restart identity recovery
**And**: this rule applies to both strictly remote-ahead and diverged histories
**And**: it does not rebase, fast-forward, reset, amend, or force-push accepted history
**And**: completed change-worktree results remain queued until integration and reverification finish

#### Scenario: unsafe base lane performs no checkpoint side effect

**Given**: the cumulative base is dirty or has an unfinished operation
**Or**: archive integration, merge, resolve, rejection review, or another change's publication owns the project base lane
**When**: upstream synchronization becomes due
**Then**: Conflux defers the entire checkpoint
**And**: it does not fetch, merge, verify, or push for the waiting result
**And**: independent per-change apply or acceptance may continue without gaining base-lane ownership

#### Scenario: completed changes publish serially

**Given**: two changes finish archive work while upstream integration is enabled
**When**: both become eligible for cumulative base integration
**Then**: only one change owns the base lane through local merge, verification, push, and remote confirmation
**And**: the other change remains waiting before base integration
**And**: after the first change is `pushed`, the second may start a fresh publication cycle

#### Scenario: stalled publication retains ordering

**Given**: change `alpha` owns the base lane and its bounded publication cycle stalls
**And**: change `beta` is ready for cumulative-base integration
**When**: no explicit retry has remotely confirmed `alpha`
**Then**: `beta` remains waiting before base integration
**And**: independent worktree apply and acceptance may continue
**And**: only successful retry confirmation or operator stop ends `alpha`'s lane ownership

### Requirement: Every cumulative base change passes explicit full reverification

When opt-in upstream integration changes the cumulative base tree, Conflux MUST run the command supplied by `--upstream-verify-command` from the cumulative base root before base integration may continue. This applies to conflict-free and agent-repaired upstream merges. After every completed change result is merged into cumulative base, Conflux MUST execute the same complete command while retaining the base lane and MUST execute it again after any later upstream tree change before that result may be pushed. Every opted-in change publication MUST verify the exact cumulative HEAD being made eligible for push. Disabled and dry-run paths MUST NOT run it.

A failed verification MAY enter bounded semantic repair through `resolve_command`, but every repair attempt MUST be followed by a successful rerun of the complete verification command. Narrative output, events, logs, or external status MUST NOT substitute for the command result.

#### Scenario: conflict-free merge has semantic failure

**Given**: Conflux completes a non-fast-forward upstream merge without textual conflict
**When**: the complete verification command exits non-zero
**Then**: Conflux blocks later base integration and push
**And**: it may invoke bounded semantic repair
**And**: it does not treat clean Git merge as completed synchronization

#### Scenario: accepted stale worktree interaction fails before publication

**Given**: a completed worktree was accepted from an older cumulative base
**And**: an upstream checkpoint changed cumulative base before that result entered base
**When**: Conflux integrates the completed result into current cumulative base
**Then**: it keeps the base lane closed and runs the complete verification command
**And**: a failure prevents that result's push, later base integration, and `pushed` terminal success

#### Scenario: remote advance after result verification causes reverification

**Given**: a completed change result passed verification after local base integration
**And**: the fresh pre-push fetch observes a newer remote revision
**When**: Conflux integrates that remote revision
**Then**: it runs the complete verification command against the new cumulative HEAD
**And**: it does not push or mark the change `pushed` until reverification succeeds

#### Scenario: restart deliberately reruns verification

**Given**: cumulative HEAD contains a locally integrated completed change or a valid upstream merge that is not yet proven reachable from the selected remote base
**When**: Conflux restarts or explicitly retries with upstream integration enabled
**Then**: it treats prior verification completion as unproven
**And**: it validates current archive, integration, trailer, ancestry, and remote evidence
**And**: it reruns the newly supplied complete verification command before publication when the change is not already remote-confirmed
**And**: surviving or deleted runtime journals, events, or logs do not change that decision

### Requirement: Opted-in run completes with a native cumulative-base push

For both finite run and persistent local TUI, `-u`/`--integrate-upstream` MUST make each completed change publish its verified cumulative HEAD to the selected remote's same-name base branch before that change reaches successful terminal state. Before local integration can become publication-pending, its cumulative-base integration commit MUST record recognizable Git trailers binding the change ID, selected remote, and base branch to required publication. These trailers MUST distinguish opted-in unpublished integration from ordinary disabled-mode terminal `merged` history even when no upstream advance created an upstream merge commit. No additional push option is required. Fetch, merge, verification, push, and confirmation MUST execute as native Conflux operations outside the AI command harness, and the push MUST remain non-force.

Immediately before each change publication, Conflux MUST fetch the selected remote and verify that its latest same-name branch revision is an ancestor of cumulative local HEAD. If the remote advances before the check, Conflux MUST suppress push and return to integration. If it advances between check and push, Conflux MUST return the non-fast-forward rejection to integration and reverification.

Push MUST use `git push --porcelain`. Conflux MUST classify routing only from machine-readable per-ref status and post-failure `git status --porcelain=v2`: non-fast-forward/fetch-first/stale-info is a race, tracked mutation or unmerged entries is repairable, and every other failure stalls without agent invocation. Human-readable stderr MUST NOT control routing. A repair agent MUST NOT execute push, alter credentials, bypass policy/hooks, or establish push success. After repository repair, Conflux MUST rerun convergence checks, complete verification, fresh fetch, and the native non-force push.

Push completion MUST be confirmed through `git ls-remote`: pushed local HEAD MUST equal the observed remote SHA or, after a further remote advance and fresh fetch, be its ancestor. Confirmation MUST emit change-scoped `PushCompleted` and transition that change to `pushed`. Local merge alone MUST NOT be the opted-in terminal success. A publication cycle MAY make multiple failed race attempts but MUST record at most one successful push for one cumulative HEAD and MUST NOT push an already confirmed HEAD again. Any in-process confirmed-HEAD record MAY suppress a repeated request only after that same process observed remote reachability; restart and ambiguous push or confirmation outcomes MUST re-observe the remote and classify ancestry, and memory MUST NOT be routing authority. Cancelled, blocked, stalled, verification-failed, push-failed, and unconfirmed outcomes MUST NOT mark the change `pushed`.

A finite run MUST emit `AllCompleted` only after every targeted successful change is remotely confirmed as `pushed`. A persistent local TUI MUST publish each completed change without scheduler drain, remain active after publication, and permit a later change to start a new publication cycle.

#### Scenario: successful change performs native publication

**Given**: upstream integration is enabled and change `alpha` is accepted, archived, and integrated into cumulative base
**And**: cumulative HEAD passes complete verification
**And**: the latest fetched remote base is an ancestor of cumulative HEAD
**When**: Conflux publishes `alpha`
**Then**: Conflux performs one native non-force push to the selected remote's same-name branch
**And**: it invokes no agent for the successful push
**And**: it emits `PushCompleted(alpha)` only after remote observation confirms cumulative HEAD is reachable
**And**: `alpha` reaches terminal status `pushed` rather than `merged`

#### Scenario: persistent TUI publishes multiple changes

**Given**: local TUI is running with upstream integration enabled
**And**: change `alpha` reaches `pushed`
**When**: the user later queues change `beta` and it completes archive and base integration
**Then**: the same shared upstream service starts a new publication cycle for the newer cumulative HEAD
**And**: `beta` reaches `pushed` only after its own remote confirmation
**And**: the TUI remains active without requiring scheduler drain between the two publications

#### Scenario: finite run completion follows change publication

**Given**: a finite opted-in run targets changes `alpha` and `beta`
**When**: `alpha` is remotely confirmed but `beta` is still waiting or publishing
**Then**: Conflux does not emit `AllCompleted` or exit successfully
**And**: completion occurs only after both successful changes are `pushed`

#### Scenario: blocked or cancelled publication never completes

**Given**: a change publication is blocked, stalled, failed, unconfirmed, or cancelled
**When**: Conflux handles that outcome
**Then**: it emits no `PushCompleted` for the change
**And**: it does not report that change or an encompassing finite run as successfully complete
**And**: it preserves resumable publication evidence when repository state permits retry

#### Scenario: option-less restart refuses marked unpublished integration

**Given**: a prior opted-in invocation locally integrated change `alpha`
**And**: its publication-required integration trailer binds a selected remote and branch
**And**: that integration is not proven reachable from the bound remote branch
**When**: Conflux starts cumulative parallel orchestration without `-u`
**Then**: startup fails before orchestration mutation
**And**: the diagnostic requires `-u` and a fresh verification command for recovery
**And**: `alpha` is not classified or displayed as terminal `merged`

#### Scenario: enabled restart resumes marked publication

**Given**: a publication-required integration for change `alpha` is not remote-reachable
**When**: Conflux restarts with matching `-u` configuration and a fresh verification command
**Then**: it derives the selected change, remote, branch, and pending publication from Git evidence
**And**: it reruns verification and publication without ordinary apply or acceptance dispatch
**And**: it emits `PushCompleted(alpha)` only after remote confirmation

#### Scenario: fresh zero-change invocation manufactures no history

**Given**: upstream integration is enabled with zero selected changes
**And**: local base has no explicit publication-required integration trailer or valid upstream recovery trailer
**When**: Conflux evaluates the invocation
**Then**: it performs initial fetch and identity validation and completes as no-work without verification, merge, push, or synthetic change terminal event
**And**: arbitrary local first-parent history and disabled-mode terminal `merged` changes do not become recovery work
**And**: a remote-only advance updates observation only and does not create a synthetic local merge

#### Scenario: zero-change recovery publishes without synthetic attribution

**Given**: upstream integration is enabled with zero selected changes
**And**: local base contains an explicit publication-required integration trailer or valid upstream recovery trailer not attributable to an active change
**When**: Conflux evaluates recovery
**Then**: it performs recovery checkpoint, complete verification, native push, and remote confirmation
**And**: it does not retroactively promote a disabled-mode terminal `merged` change
**And**: it does not manufacture `PushCompleted` for a nonexistent change

#### Scenario: remote advances before or during push

**Given**: change publication verification previously passed
**And**: another actor advances the selected remote before push completes
**When**: the fresh ancestry check or native non-force push observes the advance
**Then**: Conflux suppresses stale success and returns to bounded upstream integration and reverification
**And**: it does not force-push or mark the change `pushed`

#### Scenario: non-repairable push failure does not invoke an agent

**Given**: native porcelain push fails without a race per-ref status
**And**: post-failure porcelain-v2 status shows no tracked mutation or unmerged entries
**When**: Conflux classifies the failure
**Then**: it reports a resumable or stalled publication outcome with sanitized diagnostics
**And**: it does not inspect stderr text for routing or invoke `resolve_command`
**And**: it emits no `PushCompleted`

#### Scenario: retry observes prior push before repeating it

**Given**: Conflux pushed cumulative HEAD but stopped before recording remote confirmation
**When**: explicit retry or restart evaluates the unpublished change
**Then**: it first observes the selected remote
**And**: if cumulative HEAD is already remote-reachable, it confirms the change without another push
**And**: otherwise it resumes verification and native publication from repository evidence
