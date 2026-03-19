# cflx-run Reference

This reference expands the standard operating flow for the `cflx-run` skill.

## Preconditions

Before running `cflx run`, verify all of the following:

1. You are already on the branch that should receive the final merged work.
2. `git status` is clean.
3. The relevant OpenSpec change already exists under `openspec/changes/`.
4. The proposal commit already exists in git history.
5. The branch is reasonably up to date with its upstream when one exists.

## Minimal Execution Flow

```bash
git branch --show-current
git status
git remote -v
git fetch --all --prune
git status
cflx run
git status
```

## Suggested Review Commands

Use one or more of these after Conflux finishes:

```bash
git log --oneline --decorate -n 10
git show --stat --summary HEAD
git diff HEAD~1..HEAD
```

## Decision Rules

### If the tree is dirty

Stop before orchestration. Conflux relies on a clean base branch and clean worktree setup.

### If there is no upstream

It is acceptable to continue without `git pull` if the repository intentionally has no tracked remote branch.

### If the branch is behind upstream

Pull first so Conflux starts from the latest base branch state.

### If the proposal is not committed

Stop and complete the proposal commit before orchestration.

### If `cflx run` partially succeeds

Summarize what completed, what failed, and what remains on the base branch after the run.

## Reporting Template

Use a concise report after execution:

- Base branch used
- Whether the workspace was clean before run
- Whether remote sync was required
- Whether `cflx run` succeeded
- What commits or merge results landed
- Any follow-up actions needed
