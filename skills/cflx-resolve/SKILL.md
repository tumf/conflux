---
name: cflx-resolve
description: Conflict resolution and sequential merge guidance for Conflux parallel execution. Provides fixed rules for merge conflict resolution, pre-sync requirements, and retry continuation. CRITICAL - This skill CANNOT ask questions or request user input.
---

# Conflux Conflict Resolver

Resolve merge conflicts and complete sequential merges during Conflux parallel execution.

**CRITICAL**: This skill CANNOT ask questions to users. All decisions must be made autonomously based on available context.

## Purpose

When Conflux processes multiple changes in parallel, merge conflicts may arise when integrating change branches into the target branch. This skill provides the fixed guidance for conflict resolution, including pre-sync requirements, merge commit conventions, and safety constraints.

## Conflict Resolution Rules

- If `openspec/CONSTITUTION.md` exists, read it before conflict resolution and treat it as higher-priority project law than proposal/spec deltas.

### Safety Constraints

- Do NOT use `--no-verify` flag when committing. Always run pre-commit hooks.
- Do NOT break existing functionality unrelated to the conflicting changes.
- When resolving conflicts, preserve both sides' intent where possible.
- If shared code is modified, ensure all existing callers still work correctly.
- Do NOT remove or alter existing functionality that is not part of the conflicting changes.
- Do not use destructive commands like `reset --hard`.

### Merge Conflict Resolution

When resolving merge conflicts in listed files:

1. Examine the conflict markers in each file
2. Understand the intent of both sides of the conflict
3. Resolve by preserving both sides' intent where possible
4. If one side's changes supersede the other, prefer the more recent or more complete change
5. After resolving, run `git add` on each resolved file
6. Complete the merge commit

### Sequential Merge Protocol

When completing sequential merges into a target branch, follow this protocol for each branch in order:

#### Step 1: Pre-sync in the worktree directory
- `cd <worktree_path>`
- `git checkout <branch>`
- `git merge --no-ff -m "Pre-sync base into <change_id>" <target_branch>`
- If a conflict occurs, resolve it, `git add`, then `git commit -m "Pre-sync base into <change_id>"` to complete the merge.
- If the merge commit message is wrong, fix it with: `git commit --amend -m "Pre-sync base into <change_id>"`

#### Step 2: Final merge into target branch (in repo root)
- `cd <repo_root>`
- `git checkout <target_branch>`
- `git merge --no-ff --no-commit <branch>`
- If a conflict occurs, resolve it and `git add` the resolved files.
- **BEFORE creating the merge commit**:
  * If `openspec/changes/<change_id>/proposal.md` exists AND `openspec/changes/archive/` contains the same `<change_id>`, remove `openspec/changes/<change_id>` (the directory was resurrected by the merge and must be deleted).
  * Use `git rm -rf openspec/changes/<change_id>` to remove the resurrected directory.
- Finally, run `git commit -m "Merge change: <change_id>"` to complete the merge.

#### Step 3: Post-commit
- If a pre-commit hook modifies files and stops the commit, re-stage and re-run `git commit` with the same message.

### Commit Message Conventions

- Pre-sync merge commit subject MUST be exactly: `Pre-sync base into <change_id>`
- Final merge commit subject MUST be exactly: `Merge change: <change_id>`
- These conventions are validated by the orchestrator after resolution.

## Upstream Integration Repair Mode

This mode applies **only** when the orchestrator prompt contains
`Operation: upstream-integration`. It does not change the sequential-merge mode
above; the two modes never run together.

In this mode Conflux is integrating a remote base branch into the running
cumulative base. Conflux itself already performed the fetch and the
`git merge --no-ff` — you are a repair worker, not the workflow controller.

### What the orchestrator provides

- `Cause`: `textual conflict`, `failed verification command`, or
  `repository state blocking the cumulative-base push`
- `Selected remote` and `Cumulative base branch`
- `Local cumulative revision before integration`
- `Fetched remote SHA` (when an upstream revision is under integration)
- `Unmerged files` and the current `git status --porcelain=v2` output
- `Complete verification command` and its bounded failure output, for semantic repair
- Sanitized push diagnostics, for push repair

### What to do

1. **Textual conflict**: resolve the conflict markers preserving both the
   accepted local cumulative intent and the upstream intent, `git add` each
   resolved file, and complete the in-progress merge with `git commit` (keep the
   `Cflx-Upstream-*` trailers in the message intact).
2. **Failed verification command**: make the smallest forward change that makes
   the complete verification command pass, then commit it as a new commit on top
   of the current HEAD.
3. **Repository state blocking the push**: bring the worktree and index back to a
   clean state by committing or reverting the local mutation. Do not touch the
   remote.

### Hard bounds in this mode

- Do NOT `git commit --amend`, `git rebase`, `git reset`, `git cherry-pick` over
  existing history, or otherwise rewrite cumulative history. Repair MAY create
  new forward commits only.
- Do NOT run `git push` in any form, and never `--force`. Conflux owns the push.
- Do NOT alter credentials, remotes, or hook configuration, and do NOT bypass
  hooks with `--no-verify`.
- Do NOT remove or contradict the `Cflx-Upstream-Remote`, `Cflx-Upstream-Branch`,
  or `Cflx-Upstream-SHA` trailers on the upstream merge commit; they are the only
  restart-recovery evidence.

### How success is decided

Conflux, not you, owns retry limits and convergence. After your attempt it
revalidates repository state: forward-only ancestry from the repair-start HEAD, a
clean worktree and index, no unfinished merge, the fetched SHA still an ancestor
of HEAD, unchanged reachable identity trailers, and a successful rerun of the
complete verification command. Narrative claims of success are ignored, and a
zero exit status alone proves nothing.

## Context Provided by Orchestrator

The orchestrator injects variable context including:
- VCS-specific conflict resolution prompt prefix
- Conflicting revisions and change IDs
- VCS error output
- Current VCS status
- VCS log for conflicting changes
- Conflicting file list
- Target branch and base revision (for sequential merges)
- Worktree directory locations
- Previous attempt history (for retry continuation)

## Retry Continuation

When previous resolve attempts have failed, the orchestrator provides continuation context with:
- Previous attempt outcomes (success/failure, duration)
- Previous stdout/stderr tails
- Continuation reasons explaining why the previous attempt was insufficient

Use this context to avoid repeating the same mistakes and to focus on what was missing in previous attempts.

## Autonomous Decision Framework

When facing ambiguous conflict resolutions:

1. **Preserve both** - Keep functionality from both sides where possible
2. **Prefer completeness** - If one side is more complete, prefer it
3. **Check callers** - Ensure all existing callers still work after resolution
4. **Test after resolve** - Verify the resolved code compiles and passes tests

**Never**:
- Ask user for clarification
- Stop and wait for input
- Use destructive git commands
- Skip pre-commit hooks
