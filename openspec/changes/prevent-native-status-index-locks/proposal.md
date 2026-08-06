---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/hooks/spec.md
  - openspec/specs/release-workflow/spec.md
  - openspec/changes/archive/2026-08-03-prevent-tui-refresh-index-locks/
  - openspec/changes/archive/2026-08-04-retry-final-apply-commit-lock-contention/
  - src/vcs/git/commands/basic.rs
  - src/vcs/git/commands/commit.rs
  - src/vcs/git/commands/merge.rs
  - src/vcs/git/mod.rs
  - src/parallel/conflict.rs
  - src/execution/apply.rs
  - src/execution/state.rs
  - src/execution/archive.rs
  - src/upstream/git_ops.rs
verifications:
  - id: native-status-lock-regressions
    requirement: "Every Conflux-owned native read-only git status observation preserves its existing classification and output contract without requesting an optional index lock, while mutating Git commands retain normal locking"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Non-empty Rust unit and temporary-repository test output covering exact argv ordering across shared and direct adapters, status output fidelity, a byte-level index positive control, child-local scope, and unchanged mutating-command behavior"
    rerun: 'cargo test --lib native_git_status_optional_locks -- --list | grep -q ": test$" && cargo test --lib native_git_status_optional_locks && cargo fmt --check && cargo clippy --locked --all-targets --all-features -- -D warnings'
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Prevent native Git status index-lock contention

**Change Type**: implementation

## Premise / Context

- `prevent-tui-refresh-index-locks` already made `list_changes_with_uncommitted_files` run `git --no-optional-locks status --porcelain -u`.
- Other Conflux-owned status paths still execute plain native `git status`, including shared dirty-worktree and human-readable status helpers, conflict-resolution context capture, Apply/Archive phase classification, merge cleanliness checks, and upstream repository observations.
- A running Conflux instance was observed executing root `git status --porcelain --untracked-files=normal --ignored=no` approximately once per second; `lsof` identified one such Git process holding the root `.git/index.lock`.
- During the same run, `on_merged` waited for a pre-existing root lock to disappear, started `make bump-patch`, generated release files, and then failed when the release commit could not create `.git/index.lock`.
- The failed release left `Cargo.toml` and `Cargo.lock` visibly staged at the next version, making the base repository dirty and preventing the pending resolve from progressing.

## Problem / Context

Conflux uses native `git status` as a read-only observation, but several production paths allow Git's default optional-lock behavior. Git may then take `.git/index.lock` only to persist refreshed index stat data. A long-lived TUI or scheduler can issue those observations while `on_merged`, a release command, Apply/Archive finalization, or an operator performs an authorized index mutation.

The current `on_merged` preflight only observes whether the lock exists before hook launch. It cannot reserve the index across an arbitrary hook command, and a later Conflux status poll can acquire an optional lock after preflight succeeds. The result is self-contention: a read-oriented Conflux query can make an authorized Conflux or operator commit fail, leave a partial visible release delta, and hold later base-lane work behind a dirty repository.

Fixing only the previously identified change-list helper is insufficient because the same root and managed-worktree observations also flow through other native status implementations.

## Proposed Solution

Apply one child-command-local policy to every Conflux-owned native read-only `git status` invocation in production code: pass Git's global `--no-optional-locks` option before the `status` subcommand. Reuse shared command construction where output and error contracts permit it, and make direct execution/upstream adapters use the same exact ordering without introducing process-wide environment state.

Preserve each caller's existing semantics, including untrimmed porcelain status columns, explicit untracked/ignored modes, porcelain v2 output, pathspec scoping, and module-specific error mapping. Do not weaken index-mutating commands such as add, commit, merge, reset, checkout, tag, or release publication.

Add non-vacuous repository tests. A normal status positive control must demonstrably change complete index bytes in a stale-stat fixture; the production read-only status paths must report current state without changing those bytes. Exact argv/adapter tests must fail if `--no-optional-locks` is omitted, placed after `status`, or delivered through `GIT_OPTIONAL_LOCKS`.

## Atomic Scope Rationale

The shared dirty-state and human-readable status helpers, conflict-resolution context capture, phase classifiers, merge checks, and upstream adapter all observe repositories that can be mutated by lifecycle work. Updating only one family leaves another Conflux-owned plain status command able to recreate the same root or managed-worktree lock. They therefore form one safety invariant and must ship together.

The existing archived TUI-refresh change is a prerequisite in history, not a hard proposal dependency: its command pattern and tests are already integrated into the base, and this change can be implemented and verified from current repository code.

## Acceptance Criteria

1. Every Conflux-owned native read-only production `git status` command passes `--no-optional-locks` before `status`, including shared dirty/porcelain helpers, human-readable plain status captured for conflict-resolution prompts, commit-mode observation, merge cleanliness, Apply/Archive state classification, upstream cleanliness, and porcelain-v2 failure classification.
2. Existing already-compliant uncommitted-change monitoring remains compliant and uses the same child-local policy.
3. No implementation sets or changes process-wide `GIT_OPTIONAL_LOCKS`; child processes running mutating Git commands retain default mandatory and optional lock behavior.
4. Status results retain existing semantics: leading porcelain status columns are not trimmed where callers require them, untracked and ignored modes remain explicit where currently required, porcelain v2 remains v2, and path-scoped checks remain path-scoped.
5. Clean, staged, unstaged, deleted, renamed, untracked, ignored, and conflicted fixture states retain their existing classifications.
6. A temporary Git fixture proves a normal status command can persist an index refresh, then proves representative production status paths leave complete index bytes unchanged while returning current repository state.
7. Command-shape tests cover shared helpers and adapters that execute Git directly, and fail if the global option is absent or follows the subcommand; any user-visible command description that claims to show the exact stage-gate status command matches the changed argv.
8. A hook/release-like mutating Git command is not rewritten with optional-lock suppression, and no lock file is deleted or bypassed.
9. Existing `on_merged` diagnostics, timeout behavior, hook retry policy, release recovery, resolve queue state, and workspace-derived routing remain unchanged.
10. Added default-suite tests complete in under one second each or are marked heavy under repository policy when a real Git boundary cannot meet that limit.

## Explicit Completion Conditions

- Production native status entry points in `src/vcs/git/commands/`, `src/execution/`, `src/upstream/`, and any still-callable production helper are inventoried and either route through the shared read-only status policy or construct the same exact global-option ordering.
- The implementation contains no process-wide optional-lock environment mutation and does not alter argv for non-status Git commands.
- Tests named by `native_git_status_optional_locks` exercise non-empty unit and temporary-repository selections, including the byte-level positive control and untrimmed porcelain fidelity.
- Regression tests cover every distinct production command-construction adapter rather than only the previously fixed TUI change-list helper; they inspect argv construction or shared-policy use instead of matching `git status` text in test fixtures, diagnostics, or prompt prose.
- `cargo test --lib native_git_status_optional_locks`, `cargo fmt --check`, and `cargo clippy --locked --all-targets --all-features -- -D warnings` pass.

## Out of Scope

- Repairing, committing, tagging, pushing, resetting, or otherwise resolving the currently staged release-version delta.
- Deleting `.git/index.lock` or inferring that a lock is stale from age alone.
- Preventing contention from IDEs, shell integrations, operators, agent-supplied commands, or other external Git processes.
- Adding a cross-process mutex around every Git operation or serializing all read-only observations behind the base-mutating lane.
- Changing `on_merged` lock-wait timeout, retry count, failure transition, merge-queue promotion, or hook ordering.
- Retrying arbitrary release, merge, archive, or hook failures.
- Changing status classification, workflow routing, release ownership, or repository mutation authorization.
- Rewriting test-only Git fixture commands that are not used by production behavior.
- Suppressing opportunistic index refreshes performed by non-status read-only Git commands such as worktree-scoped `git diff`; this status-specific policy does not claim that `--no-optional-locks` controls those paths, and any observed diff-path contention requires a separately scoped mechanism.
