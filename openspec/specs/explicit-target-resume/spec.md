### Requirement: Explicit parallel targets resolve from repository evidence

A cumulative parallel run with explicit change IDs MUST classify every requested ID before change-worktree creation or reuse registration as active, already completed in base, resumable from a valid cflx-managed worktree, or unknown. Classification MUST use workspace files, workspace Git state, and captured base-branch tree comparison. Ordinary runs classify against the captured current local base; real upstream-enabled runs MUST first complete the mandatory initial upstream checkpoint and then classify against the resulting cumulative base. Server state, runtime journals, lifecycle events, logs, and commit subjects MUST NOT establish a classification.

#### Scenario: repeated target set skips integrated completion

**Given**: an earlier invocation requested `change-a` and `change-b`
**And**: `change-a` is archived and integrated into the captured base branch
**And**: `change-b` remains active or resumable
**When**: the same explicit target set is submitted again
**Then**: Conflux classifies `change-a` as already completed
**And**: it continues `change-b` without reporting `change-a` as unknown
**And**: it does not dispatch or reopen `change-a`

#### Scenario: truly unknown target still fails

**Given**: a requested ID has no active entry, base-integrated archive evidence, or valid managed-worktree evidence
**When**: Conflux resolves all explicit targets
**Then**: it reports the ID as unknown
**And**: it fails before creating, deleting, or mutating worktrees

### Requirement: Base completion requires archive and active-directory tree evidence

An explicit target MUST be classified as already completed only when the selected base commit tree contains an exact or date-prefixed archive entry for the ID and does not contain the corresponding active change directory. Base evidence MUST preserve typed distinctions among `Completed`, `NotCompleted`, `Contradictory`, and `EvidenceError`; contradiction or Git/branch read failure MUST NOT collapse into unknown. A commit subject, runtime event, external status, uncommitted working-copy archive, or archive entry outside the selected base tree MUST NOT establish completion.

#### Scenario: archive entry and absent active directory prove completion

**Given**: the captured base branch tree contains `openspec/changes/archive/<date>-change-a`
**And**: it does not contain `openspec/changes/change-a`
**When**: `change-a` is explicitly requested
**Then**: Conflux classifies it as already completed

#### Scenario: contradictory base tree fails safely

**Given**: the captured base branch tree contains both an archive entry and active directory for the requested ID
**When**: Conflux resolves the target
**Then**: it reports contradictory repository evidence
**And**: it does not classify the target as completed or create a replacement workspace

#### Scenario: uncommitted working-copy archive is not completion

**Given**: the base working copy contains an uncommitted archive directory for the requested ID
**And**: the selected base commit tree does not contain that archive entry
**When**: Conflux checks completion evidence
**Then**: it does not classify the target as completed
**And**: absent valid managed-worktree evidence, the successful tree inspection leaves the target unknown

#### Scenario: evidence command failure is not unknown

**Given**: Git cannot read the captured base branch tree
**When**: Conflux checks completion evidence
**Then**: it reports an evidence error distinct from unknown ID
**And**: it performs no workspace mutation

### Requirement: Managed worktree resume requires workspace-local identity

A discovered worktree MUST be eligible for explicit-target resume only when it is cflx-managed and existing workspace phase detection finds readable file/Git evidence for the requested active or archived change. Workspace path or branch naming alone MUST NOT establish resume eligibility. When the active OpenSpec list and a valid managed worktree both identify the requested ID, active metadata MUST establish the target classification and existing scheduler resume discovery MAY reuse the worktree; the candidate worktree MUST NOT replace or suppress the active target.

#### Scenario: archived but unmerged worktree resumes

**Given**: the active base tree no longer lists the target as active
**And**: a managed worktree contains valid archived-not-integrated evidence for that target
**When**: the target is explicitly requested
**Then**: Conflux registers the worktree and routes it through existing archive/base-integration resume behavior
**And**: it does not classify it as unknown or create a new workspace

#### Scenario: matching name without content is rejected

**Given**: a worktree or branch name appears to match the requested ID
**And**: its file/Git state does not identify a supported phase for that target
**When**: Conflux resolves the target
**Then**: it reports invalid workspace evidence
**And**: it does not reuse, delete, or replace the worktree

### Requirement: Explicit target classifications preserve ordering and diagnostics

Conflux MUST retain original requested order across active, resumable, and already-completed classifications. Duplicate IDs MUST remain invalid. Duplicate, unknown, contradictory, and unreadable-evidence diagnostics MUST be aggregated before mutation rather than failing after partially preparing workspaces.

#### Scenario: mixed target classification retains request order

**Given**: an explicit request orders IDs as completed-a, active-b, resumable-c
**When**: classification succeeds
**Then**: result evidence retains that requested order
**And**: scheduler work contains active-b then resumable-c as applicable
**And**: already-completed output contains completed-a

#### Scenario: multiple invalid targets report together

**Given**: a request contains a duplicate, an unknown ID, and an invalid worktree candidate
**When**: Conflux resolves all targets
**Then**: one deterministic diagnostic identifies every class of failure
**And**: no workspace mutation has occurred

### Requirement: Resume options and frontends have explicit boundaries

The repository-evidence resolver MUST apply to ordinary and upstream-enabled cumulative parallel explicit-target runs. A real upstream-enabled run MUST complete its initial upstream checkpoint before classification; parallel dry-run MUST perform no network fetch and MUST classify read-only against the current local base. `--no-resume` MUST preserve already-completed classification but MUST reject targets whose only valid evidence is a resumable worktree without deleting it. `--all` and serial target behavior MUST remain unchanged. An all-already-completed upstream-enabled result MUST NOT bypass the existing zero-change recovery/finalization path when recognized unpublished cumulative or upstream history exists.

#### Scenario: no-resume retains base completion

**Given**: one target is already completed in base
**And**: another target exists only as a resumable worktree
**When**: the explicit parallel run uses `--no-resume`
**Then**: the completed target remains a successful skip
**And**: Conflux rejects the worktree-only target before deleting its evidence

#### Scenario: dry-run reports without mutation

**Given**: explicit targets include active, completed, resumable, and unknown IDs
**When**: parallel dry-run resolves them
**Then**: it reports each classification and the unknown error deterministically
**And**: it performs no worktree creation, reuse registration, cleanup, merge, or archive mutation

#### Scenario: upstream mode classifies after initial checkpoint

**Given**: `-u` is enabled and the supervisor resubmits the original explicit target set
**When**: Conflux starts the real run
**Then**: it completes the mandatory initial upstream checkpoint
**And**: it runs the shared repository-evidence resolver against the resulting cumulative base before change-worktree creation or reuse registration
**And**: no server-provided remaining-target calculation is required

#### Scenario: all-completed upstream recovery still finalizes

**Given**: every requested target classifies already completed after the initial checkpoint
**And**: repository evidence contains recognized unpublished cumulative or upstream recovery history
**When**: Conflux handles the empty scheduler work set
**Then**: it enters the existing zero-change recovery/finalization path
**And**: it performs required verification, native push, and remote confirmation before reporting completion

#### Scenario: upstream dry-run performs no fetch

**Given**: `-u` and explicit targets are supplied with parallel dry-run
**When**: Conflux resolves the targets
**Then**: it classifies against the current local base and reports that basis
**And**: it performs no initial fetch, merge, worktree registration, verification, or push
