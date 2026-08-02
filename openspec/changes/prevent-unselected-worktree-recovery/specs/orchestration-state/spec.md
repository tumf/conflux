## MODIFIED Requirements

### Requirement: Archived dirty workspaces remain scheduler-recoverable after archive finalization failure

When a parallel workspace has already moved a change into `openspec/changes/archive/` but archive commit finalization remains incomplete, repository evidence SHALL continue to classify that workspace as recoverable rather than permanently terminal solely because an earlier attempt failed. Workspace file state, Git state, and base-tree comparison remain authoritative for the resume phase, and no durable external resume state is required.

Recoverable evidence alone MUST NOT create current-process operator intent. The scheduler SHALL re-own archived-dirty repair on a later cycle or restarted run only after the change is an explicit invocation target, has current reducer `QueueIntent::Queued`, or has applicable reducer-owned `ResolveWait` or `RejectWait` lane intent. An unrequested archived-dirty workspace MUST NOT prevent an otherwise drained run from becoming idle or complete.

#### Scenario: Restart preserves recoverability without automatic execution

**Given**: `alpha` is archived-dirty and not merged in its preserved workspace
**When**: Conflux restarts with no execution marks, queue intent, or lane-wait intent for `alpha`
**Then**: Workspace evidence still classifies `alpha` as recoverable
**And**: The scheduler does not execute or mutate `alpha`
**And**: An otherwise drained run may remain idle or complete

#### Scenario: Explicit intent reclaims archived-dirty workspace

**Given**: `alpha` is archived-dirty and not merged
**And**: A new invocation explicitly targets `alpha` or accepted reducer queue intent exists for `alpha`
**When**: The scheduler resolves eligible work
**Then**: It reclaims `alpha` as archive-finalization recovery work
**And**: It derives the resumed phase from current repository evidence
**And**: It does not require durable external resume state

#### Scenario: Archived dirty recovery does not require full archive command rerun

**Given**: Archive file movement for `alpha` is already correct
**And**: Only archive commit finalization remains incomplete
**And**: `alpha` has current explicit execution intent
**When**: Conflux resumes recovery for `alpha`
**Then**: It resumes archive finalization repair rather than rerunning the full archive command unnecessarily
**And**: It verifies that archive file state has not regressed

#### Scenario: Revoked intent leaves recoverable evidence untouched

**Given**: `alpha` remains archived-dirty after its ordinary queue intent is removed or dequeued
**When**: A later scheduler cycle sees its preserved worktree
**Then**: The workspace remains recoverable evidence
**And**: It is not re-owned as execution work until explicit intent is supplied again
