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

### Canonical Spec Diff Inspection

After reviewing commits, inspect the canonical spec diffs under `openspec/specs/**` to verify that spec promotion occurred correctly:

```bash
# Identify all canonical spec files that changed in this run
git diff HEAD~1..HEAD -- openspec/specs/

# For a more targeted view of a specific spec
git diff HEAD~1..HEAD -- openspec/specs/<spec-name>/spec.md
```

If multiple changes were archived in a single run, use the archived change directories to identify which canonical specs each change was responsible for:

```bash
# List archived changes to know what landed
ls openspec/changes/archive/

# For each archived change, identify its spec deltas
python3 openspec/scripts/cflx.py show <change-id> --json --deltas-only 2>/dev/null || \
  cat openspec/changes/archive/<change-id>/proposal.md
```

### Review Checklist

- Confirm the branch is still the expected base branch.
- Confirm the resulting merge or commits look correct.
- Identify which changes were archived during this run.
- **For each archived change that landed**: name the canonical spec files changed by that change and confirm they appear in the `openspec/specs/**` diff. This per-change mapping is required in the run summary.
- **Anomaly flag — spec-only change with empty canonical diff**: If a landed change is classified as `spec-only` and the canonical `openspec/specs/**` diff shows no files attributable to that change, report this as anomalous. Do not treat the run as fully healthy until the missing spec promotion is explained.
- Call out any failures, skipped changes, or conflicts reported by Conflux.

### Worked Example: Combining Commit and Spec Review

A thorough post-run review uses two complementary layers:

**Layer 1 — Commit review** answers "what code or documentation landed?":

```bash
git log --oneline --decorate -n 10
git diff HEAD~1..HEAD
```

This confirms that the expected commits are present and that no unexpected files were changed.

**Layer 2 — Canonical spec review** answers "which specs were promoted and are they correct?":

```bash
git diff HEAD~1..HEAD -- openspec/specs/
```

For each archived change, cross-check the spec delta in the change proposal against what actually appeared in the canonical specs diff. If the proposal said a spec would be added or updated but `git diff` shows no canonical spec change, this is a promotion gap and must be investigated before the run is signed off as healthy.

A complete run summary names each landed change and, for each one, lists the canonical spec files it touched (or explicitly notes that none were expected).

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
git diff HEAD~1..HEAD -- openspec/specs/
```

## Reference Files

- **[references/cflx-run.md](references/cflx-run.md)** - Detailed execution guidance for preparing and reviewing `cflx run`
