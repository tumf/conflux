### Requirement: Interrupted Apply preserves workspace-local progress

After cancellation or absolute runtime-limit expiry of an active managed-worktree Apply, Conflux MUST first prove that the owned process group is quiescent and then MUST preserve dirty staged, unstaged, and untracked workspace progress through the existing Conflux-owned WIP snapshot path. The interruption outcome MUST stop the active run without same-run automatic redispatch. If cleanup or snapshot creation fails, Conflux MUST retain workspace contents, return actionable diagnostics, and MUST NOT report successful preservation or dispatch Acceptance.

#### Scenario: Operator cancellation preserves dirty Apply progress

**Given**: a managed Apply command has changed staged, unstaged, or untracked files
**When**: the active Apply is cancelled
**Then**: Conflux closes command admission and terminates the owned process group
**And**: Conflux proves process-group quiescence before repository mutation
**And**: Conflux creates a WIP snapshot containing the dirty workspace progress
**And**: the active run stops without automatically redispatching Apply

#### Scenario: Runtime-limit expiry preserves dirty Apply progress

**Given**: a managed Apply command has changed the workspace
**And**: the command reaches its absolute runtime limit
**When**: process-group cleanup confirms quiescence
**Then**: Conflux creates one WIP snapshot through the existing workspace manager
**And**: the runtime-limit outcome remains non-retryable within the active run
**And**: Acceptance is not dispatched

#### Scenario: Restart derives continuation from the preserved workspace

**Given**: an interrupted Apply created a WIP snapshot
**When**: Conflux starts in a fresh process after external state and logs are removed
**Then**: the next action is derived from workspace files, Git history, and base comparison
**And**: the change resumes as existing Apply work rather than an unstarted change

#### Scenario: Snapshot failure retains recoverable files

**Given**: an interrupted Apply is dirty and its process group is quiescent
**When**: WIP snapshot creation fails
**Then**: Conflux leaves the workspace and index contents available for recovery
**And**: it returns non-zero with snapshot diagnostics
**And**: it does not report successful interruption recovery
