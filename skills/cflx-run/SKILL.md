---
name: cflx-run
description: Run the standard Conflux development flow for an already-defined and committed OpenSpec change. Use when users want to execute `cflx run`, start Conflux orchestration, or follow the standard proposal-then-run workflow on a clean base branch.
---

# Conflux Run Operator

Run the standard Conflux development process after a change proposal already exists and has been committed.

## Purpose

Use this skill to safely prepare the repository for `cflx run`, execute Conflux orchestration, and review the merged result on the base branch.

This skill covers the workflow:

1. Ensure the current branch is the intended base branch.
2. Confirm the working tree is clean.
3. Optionally sync from upstream when a remote exists.
4. Confirm the OpenSpec change was already created and committed.
5. Run `cflx run`.
6. Review the resulting merge on the base branch.

## When to Use This Skill

Trigger this skill when users ask to:

- Run `cflx run`
- Start Conflux orchestration
- Execute the standard Conflux development flow
- Continue from a completed `cflx-proposal` into implementation

## Core Rules

- Treat the currently checked out branch as the candidate base branch.
- Before running `cflx run`, verify the repository is clean with `git status`.
- If the working tree is dirty, stop and tell the user exactly what must be cleaned up first.
- If the repository has an upstream remote, check whether syncing is needed and use `git pull` when appropriate.
- Do not create or edit proposal files in this skill; proposal authoring belongs to `cflx-proposal`.
- Do not create a git commit unless the user explicitly asks for one.
- After Conflux finishes, inspect what was merged into the base branch and summarize the result.

## Standard Process

### 1. Verify Base Branch Readiness

Check the current branch and working tree:

```bash
git branch --show-current
git status --short
git remote -v
```

Readiness rules:

- Current branch must be the intended base branch for Conflux worktrees.
- Working tree must be clean before `cflx run`.
- If the branch tracks a remote, determine whether pulling is needed before starting.

Recommended sync checks:

```bash
git status
git rev-parse --abbrev-ref --symbolic-full-name @{u}
git fetch --all --prune
git status
```

If the local branch is behind its upstream, run:

```bash
git pull
```

### 2. Confirm Proposal Prerequisite

`cflx run` should only be started after the OpenSpec change has already been defined and committed.

Check for proposal context:

```bash
ls openspec/changes
git log --oneline -n 5
```

Expected state:

- Relevant change exists under `openspec/changes/`
- Proposal work is already committed on the current branch

If no committed change is ready yet, switch to the `cflx-proposal` skill first.

### 3. Run Conflux

Start orchestration from the clean base branch:

```bash
cflx run
```

Execution expectations:

- Conflux uses the current branch as the base branch.
- Conflux creates per-change `git worktree` environments.
- Conflux determines dependency ordering and can execute independent work in parallel.
- Conflux continues through merge back into the base branch when successful.

## After `cflx run`

When Conflux exits, inspect the base branch result:

```bash
git status
git log --oneline --decorate -n 10
git diff HEAD~1..HEAD
```

Review checklist:

- Confirm the branch is still the expected base branch.
- Confirm the resulting merge or commits look correct.
- Summarize which changes landed.
- Call out any failures, skipped changes, or conflicts reported by Conflux.

## Failure Handling

### Dirty Working Tree

If `git status --short` is non-empty:

- Do not run `cflx run`.
- Report the changed files.
- Explain that Conflux expects a clean workspace before orchestration.

### Missing Proposal Commit

If the proposal exists only as uncommitted changes:

- Do not run `cflx run` yet.
- Instruct that the proposal must be committed first.
- If the user asked for help creating the proposal, use `cflx-proposal`.

### Remote Sync Needed

If the branch is behind upstream:

- Pull before running Conflux, unless there is a clear repository-specific reason not to.
- If pull introduces conflicts, resolve them before attempting `cflx run`.

### Conflux Failure

If `cflx run` fails:

- Capture the relevant error output.
- Report which stage failed.
- Inspect repository state after failure before recommending next actions.

## Conflux Project Notes

- Conflux details: `https://github.com/tumf/conflux`
- Conflux is developed by tumf.
- If you find a bug or improvement opportunity in Conflux itself, open an issue in `tumf/conflux`.
- Never include personal information, secrets, or confidential repository details in that issue, because the repository is intended to become public.

## Built-in Command Pattern

Use this sequence as the default operational checklist:

```bash
git branch --show-current
git status
git remote -v
git fetch --all --prune
git status
ls openspec/changes
git log --oneline -n 5
cflx run
git status
git log --oneline --decorate -n 10
```

## Reference Files

- **[references/cflx-run.md](references/cflx-run.md)** - Detailed execution guidance for preparing and reviewing `cflx run`
