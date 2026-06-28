---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/orchestration-state/spec.md
  - src/cli.rs
  - src/orchestrator.rs
  - src/parallel_run_service.rs
  - src/parallel/merge.rs
  - src/parallel/queue_state.rs
  - src/vcs/git/mod.rs
  - src/vcs/git/commands
---

# Add Push Post-Archive Mode

**Change Type**: implementation

## Premise / Context

- `cflx run --parallel` currently completes archived changes by integrating each change worktree branch into the original base branch.
- The current merge path flows through `src/parallel/queue_state.rs`, `src/parallel/merge.rs`, and the Git workspace manager in `src/vcs/git/mod.rs`.
- The user wants an alternate run mode where Conflux does not merge into base and instead pushes the completed change branch to a remote.
- The user explicitly constrained the option to `--push [remote]`; selecting or overriding the destination branch is not supported.
- `openspec/CONSTITUTION.md` requires workflow-control decisions to remain derivable from workspace and git state, with no new durable out-of-worktree control state.

## Problem / Context

Conflux is useful both for self-integrating completed changes into the current base branch and for preparing reviewed branches for another system or human to merge. Today the post-archive terminal action is effectively fixed to base-branch merge. Teams that want Conflux to produce remote branches instead must either let Conflux merge locally and undo that integration, or manually push preserved worktree branches outside the workflow.

The requested behavior needs to preserve the existing apply/acceptance/archive pipeline while replacing only the final post-archive integration action. It must not allow branch override syntax because the destination branch must stay the same as the local change branch.

## Proposed Solution

Add a `cflx run --push [remote]` option for parallel execution. When enabled, Conflux will run the normal apply, acceptance, and archive pipeline in worktrees. After archive completion, instead of checking out the original branch and merging the worktree branch into base, Conflux will push the completed local change branch to the selected remote using an equivalent of:

```bash
git push <remote> <branch>:<branch>
```

The remote defaults to `origin` when `--push` is provided without a value. Values containing `:` are rejected with a clear CLI error because branch selection is not part of this mode.

Successful push becomes a terminal pushed outcome for reducer/UI/Web/CLI reporting. Push failure preserves the worktree and branch for operator inspection and retry. Existing merge mode remains the default when `--push` is absent.

## Acceptance Criteria

- `cflx run --parallel --push` uses remote `origin` and does not merge completed change branches into the base branch.
- `cflx run --parallel --push upstream` pushes completed change branches to remote `upstream`.
- `cflx run --parallel --push origin:main` fails before execution with a clear message that branch selection is not supported.
- The push operation sends the local change branch to the same-named remote branch.
- Successful push cleanup mirrors successful merge cleanup where safe: the completed worktree is removed after the remote push succeeds.
- Push failure leaves the worktree/branch intact and reports an actionable error.
- `on_merged` hooks are not executed in push mode because no base merge occurred.
- Reducer, CLI, TUI, and web state can represent pushed changes without labeling them merged.
- Existing merge behavior is unchanged when `--push` is not provided.

## Explicit Completion Conditions

The change is complete when repository evidence shows:

- CLI parsing accepts `--push` with zero or one remote argument, defaults to `origin`, and rejects colon-containing values.
- A post-archive action/config value is propagated from CLI run arguments into `ParallelRunService` and all spawned post-archive/retry executors.
- The final post-archive path branches between merge and push without duplicating apply/accept/archive behavior.
- Git push uses the local worktree branch name as both source and destination ref.
- Push success emits pushed-specific events/state and does not emit merge-completed or run `on_merged` hooks.
- Push failure emits pushed-specific failure/error evidence and preserves the workspace.
- Unit and integration tests cover parser behavior, action propagation, successful push to a local bare remote, branch-override rejection, no base merge, and failure preservation.
- `cflx openspec validate add-push-post-archive-mode --strict --evidence warn` passes before implementation handoff, and archive gate passes before archive.

## Out of Scope

- Supporting `remote:branch`, `--push-branch`, branch prefixing, or destination branch renaming.
- Supporting push mode for serial execution.
- Creating pull requests on the remote provider.
- Force-pushing, deleting remote branches, or changing remote tracking configuration.
- Adding durable out-of-worktree workflow-control state for push retry decisions.
