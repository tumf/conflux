---
change_type: implementation
priority: high
dependencies: []
references:
  - "openspec/CONSTITUTION.md"
  - "openspec/specs/cli/spec.md"
  - "openspec/specs/parallel-execution/spec.md"
  - "openspec/specs/git-sync/spec.md"
  - "openspec/specs/runtime-state/spec.md"
  - "src/cli.rs"
  - "src/main.rs"
  - "src/orchestrator.rs"
  - "src/parallel/merge.rs"
  - "src/parallel/conflict.rs"
  - "src/vcs/git/commands/merge.rs"
verifications:
  - id: upstream-integration-tests
    requirement: The opt-in upstream checkpoint preserves default behavior, exclusively owns the base lane, integrates with non-fast-forward merge, delegates only repair to resolve_command, reverifies every changed tree, recovers from Git evidence, and rejects stale non-force pushes.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/e2e_git_worktree_tests.rs
    evidence: cargo test output for upstream_integration unit and integration cases
    rerun: cargo test upstream_integration
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Change: integrate upstream base changes during an opted-in run

**Change Type**: implementation

## Problem / Context

`cflx run` can process many OpenSpec changes and continuously merge accepted results into one cumulative base branch. During a long run, another actor can advance the corresponding remote base branch. Existing per-change pre-sync updates an individual change worktree from the current local base, while server-mode `git-sync` reconciles a separate bare repository; neither refreshes the cumulative base during a running parallel CLI workflow.

Automatic synchronization would change established behavior and could unexpectedly fetch, merge, run repository commands, or require credentials. The existing behavior must therefore remain the default, with cumulative upstream integration enabled only by an explicit `cflx run` CLI option.

The Conflux Constitution requires workflow routing to remain derivable from workspace files, workspace Git state, and base-branch tree comparison. Upstream integration cannot depend on server DB state, an external checkpoint, or another process mutating the managed workspace.

## Proposed Solution

Add an opt-in cflx-owned upstream integration checkpoint to cumulative parallel run mode.

The CLI contract is:

- `cflx run -u` and `cflx run --integrate-upstream` enable the checkpoint for remote `origin`;
- `cflx run -u <remote>` and `cflx run --integrate-upstream=<remote>` enable it for the named remote;
- omitting `-u`/`--integrate-upstream` preserves the current execution path and performs no new fetch, upstream merge, reverification, or upstream lifecycle event;
- `-u` is an exact short alias of `--integrate-upstream` and is scoped to `RunArgs`;
- the option owns the complete bidirectional cumulative-base synchronization cycle: initial fetch validation, safe-point integration, full verification, and one final non-force push to the selected remote's same-name base branch;
- the option is valid only for cumulative parallel run mode and is rejected before workspace mutation when combined with serial mode, detached HEAD, a missing remote/ref, pre-existing local-only base commits, or per-change `--push` behavior;
- enabling the option requires an explicit `--upstream-verify-command <command>` so Conflux can enforce the complete repository gate after a changed tree;
- dry-run validates option compatibility and local remote/base identity without performing fetch, merge, resolve, verification, or push.

At a project base-lane safe point, Conflux shall:

1. pause new base-lane integration while allowing independent per-change worktree apply/acceptance to continue;
2. fetch the selected remote and resolve the branch with the same name as the checked-out cumulative base branch;
3. no-op when the fetched revision is already integrated;
4. integrate every remote change with `git merge --no-ff <fetched-sha>`, including strictly remote-ahead history;
5. classify merge outcomes from exit status, `MERGE_HEAD`, and unmerged index entries rather than localized output text;
6. invoke the existing bounded `resolve_command` agent only for textual conflict or bounded semantic repair, never for the ordinary fetch/ancestry/merge operation;
7. run the explicit full verification command after every actual upstream tree change;
8. stop in a resumable repository-derived state when resolution or verification cannot converge;
9. after all selected changes are integrated and verified, repeat fetch and ancestry validation and perform the final same-name cumulative-base push outside the AI command harness;
10. on push failure, classify concurrent advancement and credential/transport failures before deciding whether to refetch, fail directly, or invoke `resolve_command` for repository-repairable evidence;
11. after any agent repair, re-establish convergence and full verification, then let Conflux—not the agent—retry the non-force push.

Conflux remains the sole writer and workflow controller. `resolve_command` is a repair worker: it receives upstream-specific context, but Conflux owns Git operation selection, push execution, retry limits, convergence checks, verification, and continuation decisions. The agent MUST NOT run or claim success for the final push.

## Acceptance Criteria

1. Without `-u`/`--integrate-upstream`, run mode performs no new upstream checkpoint behavior and remains backward compatible.
2. `-u` and `--integrate-upstream` produce identical validated runtime configuration; absent remote values resolve to `origin`, and explicit values select that remote.
3. Invalid mode/option combinations, detached HEAD, missing remote/ref, and missing `--upstream-verify-command` fail before workspace mutation.
4. An unsafe or dirty base lane defers the whole checkpoint, including fetch; independent per-change apply/acceptance may continue, but their completed results cannot enter base until the checkpoint releases the lane.
5. An unchanged, local-ahead, or already-integrated fetched revision is a deterministic no-op that invokes no merge, `resolve_command`, or reverification.
6. Remote-ahead and diverged revisions are integrated with a non-fast-forward merge commit; accepted cumulative history is not rebased, reset, amended, or force-pushed.
7. Ordinary upstream merge is performed by Conflux. Only textual conflict or failed semantic verification may invoke the existing bounded `resolve_command` agent.
8. Git conflict classification uses repository state, not human-readable Git output strings.
9. Resolve success requires no unmerged entries, no unfinished merge, and ancestry of the fetched remote SHA; repair may add forward commits but may not rewrite cumulative history.
10. Every actual upstream tree change runs the configured full verification command; failure blocks further base integration and push.
11. After restart, an unpushed upstream merge is treated as unverified and reruns the full verification command; logs, events, and runtime journals cannot establish completion.
12. A successful opted-in run performs one final non-force push of the verified cumulative base to the selected remote's same-name branch; `-u` does not require a second push-enabling option.
13. Immediately before final push, Conflux fresh-fetches and checks ancestry. A later non-force push rejection returns to the bounded checkpoint flow rather than forcing or declaring success.
14. Fetch, merge, verification, and push execute as Conflux-owned native operations outside the AI command harness. A push failure invokes `resolve_command` only when repository evidence is repairable; credential, permission, transport, hook-policy, and remote-service failures are reported directly without asking an agent to guess.
15. After agent repair, Conflux reruns convergence checks and full verification and performs the retry itself; the agent never executes the push.
16. Serial mode, TUI, server bare-repository `git-sync`, existing per-change pre-sync, and `PushToRemote` behavior remain unchanged.
17. Operator output identifies enabled remote, fetched/local revisions, no-op, integration, resolving, reverifying, pushing, push-failed, stalled, retry, and completed outcomes without becoming routing authority.

## Explicit Completion Conditions

- `RunArgs` exposes `-u`/`--integrate-upstream[=<remote>]` and `--upstream-verify-command`, with parser and startup validation tests proving alias equivalence, default-off behavior, `origin` defaulting, explicit remote selection, dry-run suppression, and invalid combinations.
- The parallel scheduler exposes one repository-verifiable base-lane safe-point predicate and tests prove independent worktree execution may continue while base integration is paused.
- Git/VCS code supports fetch, fetched SHA resolution, ancestry classification, repository-state conflict classification, and `--no-ff` merge through testable operations.
- Existing `resolve_command` receives upstream context only on repair paths and Conflux validates convergence after every invocation.
- Every changed upstream tree executes the provided full verification command; no-op and disabled paths execute none.
- Restart tests prove unpushed upstream merge commits rerun verification without external durable workflow-control state.
- Startup tests prove `-u` rejects a base containing pre-existing local-only commits, preventing unrelated local history from being pushed.
- Finalization tests prove successful opted-in runs push the verified cumulative base exactly once through native Git execution outside `AgentRunner`/the AI command harness.
- Pre-push tests prove a second remote advance and a race-time non-force rejection both return to integration and reverification; repairable repository failures may hand off to `resolve_command`, while credential/transport/policy failures do not.
- `cargo test upstream_integration`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and strict OpenSpec validation pass.

## Out of Scope

- Enabling upstream integration by default or through persistent config in this change.
- Adding the option to TUI, default no-subcommand mode, or server mode.
- Implementing conflux-server supervisor, endpoint, container, or authentication behavior.
- Allowing an external supervisor or repair agent to select or perform the ordinary upstream merge workflow.
- Rebase, force push, reset, amend, or other cumulative-history rewriting.
- Distributed leases across multiple supervisors.
- Changing existing server-mode bare-repository `git-sync`, per-change pre-sync, or `PushToRemote` semantics.
- Inferring organization-specific verification commands; the operator supplies the complete command explicitly.
