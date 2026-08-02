---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/parallel-execution/spec.md
  - openspec/specs/hooks/spec.md
  - src/tui/runner.rs
  - src/tui/orchestrator.rs
  - src/parallel_run_service.rs
  - src/vcs/git/commands/commit.rs
  - src/vcs/commands.rs
verifications:
  - id: refresh-status-lock-tests
    requirement: "Repository monitoring classifies active OpenSpec worktree changes without requesting Git optional index locks"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Rust unit and temporary-Git test output proving the production argv contract, path classification, and byte-level index non-mutation with a positive control"
    rerun: "cargo test vcs::git::commands::commit::tests"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Prevent TUI refresh status from contending on the Git index

**Change Type**: implementation

## Problem / Context

The TUI periodically calls `list_changes_with_uncommitted_files` while an `on_merged` hook may mutate and commit the same root repository. The monitoring helper currently executes `git status --porcelain -u` with Git's default optional-lock behavior, which may refresh index stat information through `.git/index.lock` even though Conflux uses the command as a read-oriented poll.

In the observed incident, the hook preflight reported no root index lock, the TUI status query was the only logged root-index-locking candidate, and the later release `git add -A` failed on `.git/index.lock`. The timing is consistent with contention from that status query, but the available logs do not record command PIDs or lock acquisition and therefore do not prove a unique lock owner or exclude external Git processes.

The repository paths in the preflight and failure message resolve to the same repository. Process-local merge guards cannot serialize the independent refresh loop or another `cflx` process monitoring the same root.

## Proposed Solution

Run only the uncommitted-change monitoring query as `git --no-optional-locks status --porcelain -u`. The global option must precede the `status` subcommand and must be scoped to this child command; process-wide environment mutation is not permitted. Repo-mutating commands and other Git helpers retain their existing locking and failure behavior.

Keep all current path classification behavior while making the command contract explicit and testable. Add an exact argv unit test plus temporary-Git regression coverage for staged and unstaged additions, modifications, deletions, untracked files, same-change renames, clean committed paths, and existing exclusions. A byte-level index non-mutation test must first prove with a control command that the fixture can cause a normal status refresh, avoiding vacuous inode or timestamp assertions.

This proposal is independent of `scope-release-bump-commit`: each can be implemented and verified without consuming repository output from the other.

## Acceptance Criteria

1. TUI refresh, parallel startup, and queue filtering use `git --no-optional-locks status --porcelain -u` through the existing shared helper.
2. Optional-lock suppression is child-command-local and does not alter repo-mutating Git commands or process-wide environment state.
3. Staged and unstaged additions, modifications, and deletions plus untracked files and same-change renames retain their existing active change classification.
4. Clean committed paths are not reported as uncommitted changes.
5. Archive entries, hidden change directories, ignored files, and unrelated repository paths remain excluded.
6. A refresh-capable fixture proves normal status can change index bytes while the production monitoring helper leaves the same class of index bytes unchanged and still reports current worktree state.
7. Hook diagnostics, retry behavior, merge serialization, and workflow-control state remain unchanged.

## Explicit Completion Conditions

- `src/vcs/git/commands/commit.rs` constructs the monitoring command with the exact argv `--no-optional-locks status --porcelain -u` and all existing callers continue through that helper.
- A unit assertion fails if the global option is omitted, moved after `status`, or replaced by process-wide environment mutation.
- Temporary-Git tests cover staged and unstaged add/modify/delete, untracked paths, same-change rename, clean committed exclusion, and archive/hidden/ignored/unrelated exclusions without ignoring Git setup failures.
- The index test compares complete index bytes and includes a positive control proving the fixture would detect a normal optional refresh.
- `cargo test vcs::git::commands::commit::tests`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings` pass.

## Out of Scope

- Proving the historical lock owner beyond the available logs.
- Preventing `.git/index.lock` contention caused by IDEs, shell integrations, unrelated Git commands, or other monitoring helpers.
- Providing a linearizable repository snapshot while another process mutates the index; later polling may converge on concurrent changes.
- Removing an existing lock, retrying arbitrary Git failures, or adding a cross-process repository mutex.
- Preserving index-refresh performance in very large repositories; disabling the optional write may repeat stat work.
- Auditing every read-oriented Git command or changing release bump commit contents.
