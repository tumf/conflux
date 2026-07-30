## ADDED Requirements

### Requirement: Upstream integration is an opt-in run-mode capability

Conflux MUST preserve existing run behavior unless cumulative parallel `cflx run` is explicitly invoked with `-u` or `--integrate-upstream`. The value-less option names MUST be exact aliases selecting remote `origin` and MUST NOT consume following positional change IDs. A named remote MUST be accepted only as `--integrate-upstream=<remote>` with `=` required; `-u <remote>` MUST NOT be supported.

Enabling upstream integration MUST require an explicit complete verification command and MUST own the final verified cumulative-base push to the selected remote's same-name branch. Conflux MUST reject unsupported or incomplete invocation combinations before mutating the workspace, including unrelated local-only first-parent integration history after the real run's initial fetch; valid cumulative change integrations with matching commit-tree archive evidence and valid upstream integration commits MUST enter recovery instead of being rejected. The option MUST NOT silently enable upstream integration in serial mode, TUI, server mode, per-change pre-sync, or `PushToRemote` workflows.

#### Scenario: option is absent

**Given**: a user starts `cflx run` without `-u` or `--integrate-upstream`
**When**: the run executes in parallel or serial mode
**Then**: Conflux follows the existing execution path
**And**: it performs no new upstream fetch, cumulative upstream merge, upstream reverification, or upstream lifecycle event

#### Scenario: short and long options are equivalent

**Given**: one invocation uses `-u` and another uses `--integrate-upstream`
**When**: neither invocation supplies a remote value
**Then**: both produce identical enabled runtime configuration for remote `origin`
**And**: a named remote is accepted only as `--integrate-upstream=<remote>`
**And**: `-u` does not consume a following change ID as a remote value

#### Scenario: invalid invocation fails before mutation

**Given**: upstream integration is requested without parallel cumulative execution, with `--push`, from detached HEAD, for a missing remote or same-name remote branch, or without a non-empty verification command
**When**: Conflux validates startup
**Then**: it rejects the invocation before fetch, merge, resolve, verification, or push mutates the workspace
**And**: the diagnostic identifies the invalid option or repository precondition

#### Scenario: dry-run suppresses side effects

**Given**: upstream integration options are valid
**When**: the user requests parallel dry-run
**Then**: Conflux validates static option compatibility, selected remote configuration, attached HEAD, repository type, base cleanliness, and non-empty verification command
**And**: it defers remote branch existence and fetched ancestry validation to the real run's initial fetch
**And**: it performs no network fetch, merge, resolve command, verification command, or push

### Requirement: Running cumulative base integrates upstream changes at base-lane safe points

When opt-in upstream integration is enabled, a cumulative parallel Conflux run MUST refresh and reconcile the selected remote branch with the same name as the checked-out cumulative base branch at deterministic project base-lane checkpoints: before first worktree dispatch, immediately before each completed result enters base, after successful scheduler drain before finalization, after a fresh pre-push fetch observes remote advance, and after race-time non-fast-forward push rejection. Conflux MUST remain the sole writer and MUST perform ordinary fetch, ancestry classification, and merge itself; an external supervisor or AI agent MUST NOT select or perform that ordinary integration workflow.

The safe point MUST require a clean cumulative base and exclusive base-lane ownership. Independent apply and acceptance commands in change worktrees MAY continue, but their results MUST NOT enter the cumulative base until the checkpoint completes. Results accumulated during one checkpoint MUST remain queued and share its single fetch. Scheduler-loop polling and time-based polling MUST NOT be introduced. The authoritative routing decision MUST be derivable from workspace files, Git state, fetched refs, and base-tree comparison.

#### Scenario: unchanged upstream is a no-op

**Given**: opt-in upstream integration is enabled
**And**: the freshly fetched remote revision is already an ancestor of cumulative local HEAD
**When**: Conflux evaluates the checkpoint
**Then**: it records an ancestry-proven no-op
**And**: it does not merge, invoke `resolve_command`, or run full reverification
**And**: proposal processing may continue

#### Scenario: upstream advance uses a non-fast-forward merge

**Given**: Conflux owns a clean project base lane
**And**: the freshly fetched remote revision is not an ancestor of cumulative local HEAD
**When**: Conflux integrates the revision
**Then**: Conflux itself runs a non-fast-forward merge of the fetched SHA
**And**: the merge commit records `Cflx-Upstream-Remote: <remote>`, `Cflx-Upstream-Branch: <branch>`, and `Cflx-Upstream-SHA: <fetched-sha>` for restart identity recovery
**And**: this rule applies to both strictly remote-ahead and diverged histories
**And**: it does not rebase, fast-forward, reset, amend, or force-push accepted history
**And**: completed change-worktree results remain queued and enter base only after integration and reverification finish

#### Scenario: unsafe base lane performs no checkpoint side effect

**Given**: the cumulative base is dirty or has an unfinished operation
**Or**: archive integration, merge, resolve, or rejection review owns the project base lane
**When**: upstream synchronization becomes due
**Then**: Conflux defers the entire checkpoint
**And**: it does not fetch or merge
**And**: independent per-change apply or acceptance may continue without gaining base-lane ownership

### Requirement: Git state authoritatively classifies upstream merge outcomes

Conflux MUST classify an upstream merge from command exit status, `MERGE_HEAD`, and unmerged index entries. Human-readable Git stdout or stderr MUST NOT be the authoritative conflict classifier.

A state with `MERGE_HEAD` and unmerged entries MUST enter upstream repair. Any other non-zero merge outcome MUST fail as a command error unless repository state independently establishes a repairable unfinished merge.

#### Scenario: localized conflict output still enters repair

**Given**: a non-fast-forward upstream merge exits non-zero
**And**: `MERGE_HEAD` exists with unmerged index entries
**When**: Git output does not contain the implementation's expected English conflict phrases
**Then**: Conflux enters upstream repair from repository evidence

#### Scenario: unrelated merge failure does not start an agent

**Given**: an upstream merge exits non-zero
**And**: no repairable merge state or unmerged index entry exists
**When**: Conflux classifies the result
**Then**: it reports a command failure
**And**: it does not invoke `resolve_command` based only on output text

### Requirement: Upstream repair uses the existing bounded resolve agent

Conflux MUST reuse the existing top-level `resolve_command` runner only when repository state proves textual repair is required or when failed full verification enters an allowed bounded semantic-repair cycle. Existing merge/conflict goal predicates MUST NOT establish semantic repair success. A dedicated upstream repair entrypoint and `cflx-resolve` upstream-integration mode MUST run in the cumulative base checkout with local/fetched revisions, selected remote/base identity, conflict or verification command/failure context, Git status, upstream-integration intent, and the prohibition on amend, rebase, reset, push, and history rewriting.

Conflux, not the agent, MUST own retry limits and convergence decisions. Textual resolution MUST NOT be complete until no unmerged entries remain, no merge is unfinished, the fetched remote SHA is an ancestor of cumulative local HEAD, and valid identity trailers remain reachable. Semantic repair additionally requires repair-start HEAD to remain an ancestor of repair-end HEAD, clean worktree/index state, unchanged valid identity trailers, and successful full verification. Repair MAY create forward commits but MUST NOT rebase, reset, amend, push, or otherwise rewrite cumulative history.

#### Scenario: ordinary integration invokes no agent

**Given**: a fetched revision is already integrated or its non-fast-forward merge completes without conflict
**When**: Conflux processes the checkpoint before verification
**Then**: it does not invoke `resolve_command`

#### Scenario: textual conflict converges

**Given**: repository state proves an upstream merge conflict
**When**: Conflux invokes `resolve_command`
**Then**: the command receives upstream-specific cause and revision context
**And**: Conflux independently verifies no unmerged entries, no unfinished merge, and fetched-SHA ancestry
**And**: agent narrative output alone does not establish success

#### Scenario: repair cannot converge

**Given**: upstream repair exhausts the existing bounded retry budget
**When**: conflicts, unfinished merge, verification failure, history rewrite, or missing fetched-SHA ancestry remains
**Then**: Conflux enters a resumable stalled outcome
**And**: it does not integrate another result into base or push

### Requirement: Every cumulative base change passes explicit full reverification

When opt-in upstream integration changes the cumulative base tree, Conflux MUST run the command supplied by `--upstream-verify-command` from the cumulative base root before base integration may continue. This applies to conflict-free and agent-repaired upstream merges. After every completed change result is merged into cumulative base, Conflux MUST execute the same complete command before releasing the base lane or allowing later base-dependent dispatch. Independently, every opted-in run MUST execute it against final cumulative HEAD immediately before push, including ancestry-proven upstream no-op runs. Disabled and dry-run paths MUST NOT run it.

A failed verification MAY enter bounded semantic repair through `resolve_command`, but every repair attempt MUST be followed by a successful rerun of the complete verification command. Narrative output, events, logs, or external status MUST NOT substitute for the command result.

#### Scenario: conflict-free merge has semantic failure

**Given**: Conflux completes a non-fast-forward upstream merge without textual conflict
**When**: the complete verification command exits non-zero
**Then**: Conflux blocks later base integration and push
**And**: it may invoke bounded semantic repair
**And**: it does not treat clean Git merge as completed synchronization

#### Scenario: accepted stale worktree interaction fails before later dispatch

**Given**: a completed worktree was accepted from an older cumulative base
**And**: an upstream checkpoint changed cumulative base before that result entered base
**When**: Conflux integrates the completed result into current cumulative base
**Then**: it keeps the base lane closed and runs the complete verification command
**And**: a failure prevents later base-dependent dispatch, another base integration, and push

#### Scenario: restart deliberately reruns verification

**Given**: a merge commit carrying valid selected-remote, base-branch, and fetched-SHA trailers is reachable from cumulative HEAD but is not yet incorporated into that selected remote base
**When**: Conflux restarts or explicitly retries
**Then**: it treats prior verification completion as unproven
**And**: it recovers the fetched SHA from the trailer and validates ancestry
**And**: it reruns the newly supplied complete verification command
**And**: surviving or deleted runtime journals, events, or logs do not change that decision

### Requirement: Unfinished upstream recovery blocks option-less continuation

When a trailer-identified Conflux upstream merge is reachable from cumulative HEAD and not incorporated into its selected remote branch, a cumulative parallel run invoked without `-u`/`--integrate-upstream` MUST refuse to continue. The diagnostic MUST require the same selected remote and a newly supplied complete verification command. No external state may supply the previous command or establish completion.

#### Scenario: operator omits upstream option after a crash

**Given**: cumulative HEAD contains an unpushed trailer-identified upstream merge
**When**: the operator starts cumulative parallel run without upstream integration
**Then**: Conflux refuses to dispatch, integrate, verify, or push more work
**And**: it instructs the operator to restart with the trailer-associated remote and an explicit verification command

### Requirement: Opted-in run completes with a native cumulative-base push

Only after the scheduler returns `DrainedSuccessfully`, the final checkpoint completes, and final cumulative HEAD passes a full verification executed immediately before push, `-u`/`--integrate-upstream` MUST make Conflux perform one final push to the selected remote's same-name base branch. No additional push option is required. Fetch, merge, verification, and push MUST execute as native Conflux operations outside the AI command harness, and the push MUST remain non-force.

Immediately before push, Conflux MUST fetch the selected remote again and verify that its latest same-name branch revision is an ancestor of cumulative local HEAD. If the remote advances before the check, Conflux MUST suppress push and return to integration. If it advances between check and push, Conflux MUST return the non-fast-forward rejection to integration/reverification.

Push MUST use `git push --porcelain`. Conflux MUST classify routing only from machine-readable per-ref status and post-failure `git status --porcelain=v2`: non-fast-forward/fetch-first/stale-info is a race, tracked mutation or unmerged entries is repairable, and every other failure stalls without agent invocation. Human-readable stderr MUST NOT control routing. A repair agent MUST NOT execute push, alter credentials, bypass policy/hooks, or establish push success. After repository repair, Conflux MUST rerun convergence checks, complete verification, fresh fetch, and the native non-force push. Push completion MUST be confirmed through `git ls-remote`: pushed local HEAD MUST equal the observed remote SHA or, after a further remote advance and fresh fetch, be its ancestor. A run MAY make multiple failed race attempts but MUST record at most one successful push and MUST NOT retry after confirmed success. `BlockedOrStalled` and `Cancelled` scheduler outcomes MUST NOT enter final checkpoint or push. `AllCompleted` MUST be emitted only after successful remote confirmation.

#### Scenario: successful run performs final push

**Given**: upstream integration is enabled and all selected changes are integrated
**And**: cumulative HEAD passes complete verification
**And**: the latest fetched remote base is an ancestor of cumulative HEAD
**When**: Conflux finalizes the run
**Then**: Conflux performs one native non-force push to the selected remote's same-name branch
**And**: it invokes no agent for the successful push
**And**: it reports completion only after remote observation confirms pushed HEAD is reachable

#### Scenario: pre-existing local history is classified from integration evidence

**Given**: after initial fetch, starting local base contains commits not reachable from the selected remote base
**When**: Conflux validates the first-parent integration spine from the remote/local merge-base to local HEAD
**Then**: it accepts `Merge change:`/`Merge changes:` commits whose own trees contain matching archive entries and omit the corresponding active change directories
**And**: it accepts merge commits whose selected-remote, base-branch, and fetched-SHA trailers match Git parent and ancestry evidence
**And**: it treats commits reachable only through those validated merges' non-first parents as payload rather than independent spine entries
**And**: it rejects every other first-parent local commit before merge/worktree mutation

#### Scenario: blocked scheduler never finalizes

**Given**: automatic scheduler work exits blocked-only or stalled, or the run is cancelled
**When**: Conflux handles the scheduler outcome
**Then**: it performs no final verification, push, or remote confirmation
**And**: it does not emit `AllCompleted`

#### Scenario: fresh zero-change run manufactures no history

**Given**: upstream integration is enabled with zero selected changes
**And**: local base has no recognized unpushed cumulative history or upstream recovery evidence
**When**: Conflux evaluates the run
**Then**: it performs initial fetch/identity validation and completes as no-work without verification, merge, or push
**And**: a remote-only advance updates observation only and does not create a synthetic local merge

#### Scenario: zero-change recovery still publishes verified cumulative history

**Given**: upstream integration is enabled with zero selected changes
**And**: local base contains recognized unpushed cumulative history or upstream recovery evidence
**When**: Conflux evaluates the run
**Then**: it performs recovery checkpoint, complete verification, native push, and remote confirmation before completion

#### Scenario: remote advances before push eligibility

**Given**: upstream integration and reverification previously passed
**And**: another actor advances the selected remote base
**When**: Conflux performs the fresh pre-push check
**Then**: it suppresses stale push
**And**: it integrates and reverifies the latest fetched revision before a later attempt

#### Scenario: remote advances between check and push

**Given**: the fresh ancestry check passes
**And**: another actor advances the remote before the non-force push completes
**When**: Git rejects the push
**Then**: Conflux returns to the upstream checkpoint under bounded retry policy
**And**: it does not force push or report completion

#### Scenario: non-repairable push failure does not invoke an agent

**Given**: native porcelain push fails without a race per-ref status
**And**: post-failure porcelain-v2 status shows no tracked mutation or unmerged entries
**When**: Conflux classifies the failure
**Then**: it reports a stalled push outcome with sanitized diagnostics
**And**: it does not inspect stderr text for routing or invoke `resolve_command`

#### Scenario: repository-repairable push failure returns to agent then native push

**Given**: native porcelain push fails without a race per-ref status
**And**: post-failure porcelain-v2 status shows tracked mutation or unmerged entries
**When**: Conflux invokes `resolve_command`
**Then**: the agent receives sanitized push and repository context but does not execute push
**And**: Conflux verifies convergence and reruns the complete verification command after repair
**And**: Conflux fresh-fetches and retries the non-force push itself
