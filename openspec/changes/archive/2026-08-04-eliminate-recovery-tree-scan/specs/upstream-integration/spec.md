## MODIFIED Requirements

### Requirement: Upstream integration is an opt-in run-mode capability

Conflux MUST preserve existing behavior unless cumulative parallel orchestration is explicitly invoked with `-u` or `--integrate-upstream`. This capability MUST be available with non-interactive `cflx run`, bare local TUI, and explicit local `cflx tui`, and all three entrypoints MUST normalize to the same invocation-scoped upstream runtime configuration and shared publication service.

The value-less option names MUST be exact aliases selecting remote `origin` and MUST NOT consume following positional change IDs. A named remote MUST be accepted only as `--integrate-upstream=<remote>` with `=` required; `-u <remote>` MUST NOT be supported.

Enabling upstream integration MUST require an explicit complete verification command and MUST make remote-confirmed cumulative-base publication part of every completed change's success contract. Conflux MUST reject unsupported or incomplete invocation combinations before mutating the workspace, including unrelated local-only first-parent integration history after a real invocation's initial fetch; valid cumulative change integrations with matching commit-tree archive evidence and valid upstream integration commits MUST enter recovery instead of being rejected. The option MUST NOT silently enable upstream integration in serial mode, remote-client TUI, server orchestration, per-change pre-sync, or `PushToRemote` workflows.

Option-less cumulative parallel startup MAY inspect bounded first-parent commit metadata to detect unfinished opted-in publication, but that recovery discovery MUST NOT inspect per-commit OpenSpec tree evidence that its trailer and reachability classification does not consume. This optimization MUST NOT weaken evidence-bearing spine validation when upstream integration is enabled.

#### Scenario: option is absent

**Given**: a user starts run or local TUI without `-u` or `--integrate-upstream`
**When**: cumulative parallel orchestration executes
**Then**: Conflux follows the existing execution path
**And**: successful local base integration terminates each change as `merged`
**And**: it performs no new upstream fetch, cumulative upstream merge, upstream reverification, push, or upstream lifecycle event

#### Scenario: option-less startup performs bounded metadata recovery discovery

**Given**: a user starts cumulative parallel run or local TUI without upstream integration
**And**: cumulative HEAD has up to the bounded recovery limit of first-parent commits
**When**: Conflux checks for unfinished upstream publication before orchestration
**Then**: it reads commit SHA, parents, and raw message in first-parent order
**And**: ordinary no-match discovery performs no per-commit OpenSpec tree inspection
**And**: the number of Git subprocesses used for no-match recovery discovery does not grow per scanned commit

#### Scenario: enabled spine validation retains tree evidence

**Given**: upstream integration is enabled after startup recovery discovery
**And**: first-parent history contains a cumulative change integration subject
**When**: Conflux validates the selected upstream spine
**Then**: it loads that commit's archive and active-change tree evidence
**And**: it rejects the integration when archive evidence is missing or the change remains active

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

### Requirement: Unfinished upstream recovery blocks option-less continuation

When a trailer-identified Conflux upstream merge is reachable from cumulative HEAD and not incorporated into its selected remote branch, a cumulative parallel run invoked without `-u`/`--integrate-upstream` MUST refuse to continue. The diagnostic MUST require the same selected remote and a newly supplied complete verification command. No external state may supply the previous command or establish completion. Recovery discovery MUST remain bounded and complete before orchestration mutation, and MUST derive trailer identity and reachability from first-parent commit metadata and local Git refs without loading unrelated per-commit OpenSpec tree evidence.

#### Scenario: operator omits upstream option after a crash

**Given**: cumulative HEAD contains an unpushed trailer-identified upstream merge
**When**: the operator starts cumulative parallel run without upstream integration
**Then**: Conflux refuses to dispatch, integrate, verify, or push more work
**And**: it instructs the operator to restart with the trailer-associated remote and an explicit verification command

#### Scenario: contradicted upstream trailer is not recovery evidence

**Given**: a bounded first-parent commit message contains upstream trailers
**And**: the trailer-recorded upstream SHA is not one of that merge commit's non-first parents
**When**: option-less startup performs recovery discovery
**Then**: Conflux does not classify that commit as valid upstream recovery evidence
**And**: this classification requires no commit-tree archive or active-change lookup
