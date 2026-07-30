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
  - id: upstream-integration-unit
    requirement: Parser, scheduler outcome, checkpoint triggering, history classification, semantic-repair predicates, and default-off routing are covered by sub-second repository-local tests.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/parallel/tests/mod.rs
    evidence: cargo test output for upstream_integration unit cases
    rerun: cargo test upstream_integration
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: upstream-integration-heavy-e2e
    requirement: Real Git repositories, worktrees, local bare remotes, hooks, process commands, restart boundaries, and final remote confirmation are covered outside the default suite.
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/e2e_git_worktree_tests.rs
    evidence: heavy-tests output for e2e_git_worktree_tests upstream_integration cases
    rerun: cargo test --features heavy-tests --test e2e_git_worktree_tests upstream_integration
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

- `cflx run -u` and `cflx run --integrate-upstream` enable the checkpoint for remote `origin` and never consume a following change ID as an option value;
- `cflx run --integrate-upstream=<remote>` enables it for the named remote, with `=` required; `-u <remote>` is not supported;
- omitting `-u`/`--integrate-upstream` preserves the current execution path and performs no new fetch, upstream merge, reverification, or upstream lifecycle event;
- `-u` is an exact short alias of `--integrate-upstream` and is scoped to `RunArgs`;
- the option owns the complete bidirectional cumulative-base synchronization cycle: initial fetch validation, safe-point integration, full verification, and one final non-force push to the selected remote's same-name base branch;
- the option is valid only for cumulative parallel run mode and is rejected before workspace mutation when combined with serial mode, detached HEAD, a missing remote/ref after initial fetch, unrelated pre-existing local-only history, or per-change `--push` behavior; normal cumulative change integrations are accepted from first-parent `Merge change:`/`Merge changes:` commits whose commit trees contain matching archive evidence, and upstream integrations are accepted from validated remote/branch/SHA trailers;
- enabling the option requires an explicit `--upstream-verify-command <command>` so Conflux can enforce the complete repository gate after a changed tree;
- dry-run validates option compatibility and local remote/base identity without performing fetch, merge, resolve, verification, or push.

Conflux shall trigger a project base-lane checkpoint at five deterministic boundaries: before first worktree dispatch, immediately before each completed change result enters base, once after normal scheduler drain before finalization, and again after either a pre-push remote advance or race-time non-fast-forward rejection. Completed results accumulated during one checkpoint remain queued and share that checkpoint's single fetch; scheduler-loop polling and time-based polling are not added.

At each checkpoint Conflux shall:

1. pause new base-dependent dispatch and base integration while allowing independent per-change worktree apply/acceptance to continue;
2. fetch the selected remote and resolve the branch with the same name as the checked-out cumulative base branch;
3. no-op when the fetched revision is already integrated;
4. integrate every remote change with `git merge --no-ff <fetched-sha>`, including strictly remote-ahead history, and record selected remote, base branch, and fetched SHA as validated trailers;
5. classify merge outcomes from exit status, `MERGE_HEAD`, and unmerged index entries rather than localized output text;
6. invoke bounded `resolve_command` only through upstream textual- or semantic-repair predicates, never through the ordinary merge success predicate;
7. run the complete verification command after every upstream integration and after every completed change result enters cumulative base, keeping the base lane closed and blocking later base-dependent dispatch until success;
8. stop in a resumable repository-derived state when repair or verification cannot converge; an unpushed trailer-identified upstream merge requires `-u` and a verification command on restart, even if the operator omitted the option;
9. after a `DrainedSuccessfully` scheduler outcome, execute final checkpoint, full verification, fresh ancestry validation, native non-force push, and remote confirmation before emitting `AllCompleted`; blocked/stalled or cancelled outcomes never push;
10. execute `git push --porcelain` and classify failures only from its machine-readable per-ref status plus `git status --porcelain=v2`: race returns to checkpoint, tracked/unmerged repository mutation may invoke repair, and every other failure stalls without an agent;
11. after any agent repair, re-establish forward-only ancestry, clean repository state, preserved upstream identity, and full verification, then let Conflux—not the agent—retry the native operation.

Conflux remains the sole writer and workflow controller. `resolve_command` is a repair worker: it receives upstream-specific context, but Conflux owns Git operation selection, push execution, retry limits, convergence checks, verification, and continuation decisions. The agent MUST NOT run or claim success for the final push.

## Acceptance Criteria

1. Without `-u`/`--integrate-upstream`, run mode performs no new upstream checkpoint behavior and remains backward compatible.
2. `-u` and `--integrate-upstream` produce identical validated runtime configuration; absent remote values resolve to `origin`, and explicit values select that remote.
3. Invalid mode/option combinations, detached HEAD, missing remote/ref after initial fetch, unrelated local-only history, and missing `--upstream-verify-command` fail before merge/worktree mutation; first-parent cumulative change integrations with matching archive tree evidence and validated upstream integrations enter recovery instead of being rejected.
4. Checkpoints run before first dispatch, immediately before completed-result base integration, after successful scheduler drain before finalization, and after pre-push/race advances; one active checkpoint batches queued completed results behind one fetch, while unsafe or dirty base state defers all checkpoint side effects.
5. An unchanged, local-ahead, or already-integrated fetched revision is a deterministic no-op that invokes no merge, `resolve_command`, or reverification.
6. Remote-ahead and diverged revisions are integrated with a non-fast-forward merge commit; accepted cumulative history is not rebased, reset, amended, or force-pushed.
7. Ordinary upstream merge is performed by Conflux. Only textual conflict or failed semantic verification may invoke the existing bounded `resolve_command` agent.
8. Git conflict classification uses repository state, not human-readable Git output strings.
9. Upstream semantic repair has a dedicated goal predicate: repair-start HEAD remains an ancestor, worktree/index are clean, no merge is unfinished, fetched SHA remains an ancestor, upstream identity trailers remain reachable and unchanged, and the full command succeeds; amend/rebase/reset/push are prohibited.
10. Every upstream tree change and every completed change result merged into cumulative base runs the configured full verification command before the base lane reopens; every opted-in run reruns it against final cumulative HEAD immediately before push.
11. Upstream merge commits carry validated remote, branch, and SHA trailers. After restart, an unpushed identified merge is treated as unverified and reruns the newly supplied full verification command; logs, events, and runtime journals cannot establish completion.
12. A cumulative parallel run without `-u` refuses to proceed when an unpushed trailer-identified upstream merge is reachable from HEAD, and directs the operator to restart with the same selected remote and an explicit verification command.
13. Only a `DrainedSuccessfully` scheduler outcome may enter final checkpoint and push; blocked/stalled and cancelled outcomes never push, and `AllCompleted` is emitted only after remote confirmation.
14. Immediately before final push, Conflux fresh-fetches and checks ancestry. A later non-force push rejection returns to the bounded checkpoint flow rather than forcing or declaring success.
15. Fetch, merge, verification, and push execute as Conflux-owned native operations outside the AI command harness. A push failure invokes `resolve_command` only when repository evidence is repairable; credential, permission, transport, hook-policy, and remote-service failures are reported directly without asking an agent to guess.
16. After agent repair, Conflux reruns convergence checks and full verification and performs the retry itself; the agent never executes the push.
17. Serial mode, TUI, server bare-repository `git-sync`, existing per-change pre-sync, and `PushToRemote` behavior remain unchanged.
18. Operator output identifies enabled remote, fetched/local revisions, no-op, integration, resolving, reverifying, pushing, push-failed, stalled, retry, and completed outcomes without becoming routing authority.

## Explicit Completion Conditions

- `RunArgs` exposes value-less `-u`/`--integrate-upstream` for `origin`, `--integrate-upstream=<remote>` for an explicit remote, and `--upstream-verify-command`, with parser and startup validation tests proving alias equivalence, default-off behavior, `origin` defaulting, explicit remote selection, dry-run suppression, and invalid combinations.
- The parallel scheduler exposes one repository-verifiable base-lane safe-point predicate and tests prove independent worktree execution may continue while base integration is paused.
- Git/VCS code supports fetch, fetched SHA resolution, ancestry classification, first-parent cumulative-history classification with commit-tree archive evidence, repository-state conflict classification, and trailer-bearing `--no-ff` merge through testable operations.
- Existing `resolve_command` runner is reused through a dedicated upstream semantic-repair goal and `cflx-resolve` mode; Conflux validates forward-only ancestry, clean state, preserved trailers, and verification after every invocation.
- Every changed upstream tree and every completed change merged into cumulative base executes the provided full verification command before later base-dependent dispatch; disabled paths execute none, and opted-in no-op paths still execute final verification before push.
- Upstream merge commits contain validated `Cflx-Upstream-Remote`, `Cflx-Upstream-Branch`, and `Cflx-Upstream-SHA` trailers, and restart tests prove identified unpushed merges rerun the newly supplied verification command without external durable workflow-control state.
- Startup tests prove unrelated first-parent local history is rejected after initial fetch, valid cumulative change integrations and upstream trailer commits enter recovery, and omission of `-u` while upstream recovery evidence exists is rejected safely.
- Scheduler tests prove deterministic checkpoint triggers, single-fetch batching, stale completed-result verification before later dispatch, explicit drained/blocked/cancelled outcomes, and `AllCompleted` after remote confirmation only.
- Finalization tests prove successful opted-in runs have at most one successful native push through Git outside `AgentRunner`/the AI command harness; blocked/cancelled/disabled/final-failure paths have no successful push, and the specified zero-change cases are deterministic.
- Pre-push tests prove a second remote advance and a race-time non-force rejection both return to integration and reverification; `git push --porcelain` and `git status --porcelain=v2` classify routing without stderr text, and only tracked/unmerged local mutation may hand off to `resolve_command`.
- `cargo test upstream_integration`, `cargo test --features heavy-tests --test e2e_git_worktree_tests upstream_integration`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and strict OpenSpec validation pass.

## Out of Scope

- Enabling upstream integration by default or through persistent config in this change.
- Adding the option to TUI, default no-subcommand mode, or server mode.
- Implementing conflux-server supervisor, endpoint, container, or authentication behavior.
- Allowing an external supervisor or repair agent to select or perform the ordinary upstream merge workflow.
- Rebase, force push, reset, amend, or other cumulative-history rewriting.
- Distributed leases across multiple supervisors.
- Changing existing server-mode bare-repository `git-sync`, per-change pre-sync, or `PushToRemote` semantics.
- Inferring organization-specific verification commands; the operator supplies the complete command explicitly.
