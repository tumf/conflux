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
5. Run `cflx run --all` or `cflx run <change-id>...` with explicit targets.
6. Review the resulting merge on the base branch.

## When to Use This Skill

Trigger this skill when users ask to:

- Run `cflx run --all`, `cflx run <change-id>...`, or legacy `cflx run --change <ids>`
- Start Conflux orchestration
- Execute the standard Conflux development flow
- Continue from a completed `cflx-proposal` into implementation

## Core Rules

- If `openspec/CONSTITUTION.md` exists, read it before running `cflx run` and treat it as higher-priority project law than proposal/spec deltas.
- Do not start orchestration from a base branch state that knowingly violates `openspec/CONSTITUTION.md`.
- Treat the currently checked out branch as the candidate base branch.
- Before running `cflx run --all` or `cflx run <change-id>...`, verify the repository is clean with `git status`.
- Use `cflx run <change-id>...` for TUI-style selected rows, `cflx run --all` for TUI `x` bulk selection, and legacy `cflx run --change a,b` only for compatibility.
- Do not run bare `cflx run`; it has no implicit all-changes target.
- If the working tree is dirty, stop and tell the user exactly what must be cleaned up first.
- If the repository has an upstream remote, check whether syncing is needed and use `git pull` when appropriate.
- Do not create or edit proposal files in this skill; proposal authoring belongs to `cflx-proposal`.
- Do not create a git commit unless the user explicitly asks for one.
- After Conflux finishes, inspect what was merged into the base branch and summarize the result.
- When delegating a change to an already-running owner, the verbs are the operator's own: `cflx client mark <change-id>` selects it and preserves every unrelated mark, and `cflx client start` is the F5 equivalent that consumes the owner's authoritative mark set. Marking admits nothing; the owner's own settlement decides that. From an MCP host the same two are `cflx_control` with action `mark` and action `start`.
- To stop one runaway proposal without disturbing the rest, use `cflx client force-stop-change <change-id>` — or `cflx_control` with action `force_stop_change` and exactly one entry in `change_ids`. It is the only control that skips the graceful SIGTERM window: the owner SIGKILLs that proposal's managed process group, confirms it was reaped, then settles it as terminal `stopped` — queue admission and execution mark cleared together, so an observing `cflx client wait` is released with `change_requires_action` rather than left watching an idle row. Unrelated changes and the process-wide run mode are untouched, completed worktree effects are preserved (`effects_rolled_back: false`), and the success outcome is `stopped`. It is never a way to stop everything — that is `cflx client force-stop`.
- Nothing subscribes you to completion automatically. If you want to be told when a proposal finishes, ask explicitly: `cflx client subscribe set|get|clear` in a shell, or the `cflx_subscribe` MCP tool when the host speaks MCP and has no shell. Both reach the same owner-side registry, so neither requires the other.
- A subscription is keyed by the *proposal*, so register it whenever you like — before marking, after starting, or after the work already settled. Each new execution episode of that proposal delivers once.
- A subscription fires on *execution* completion, not process completion. The TUI stays alive after the work finishes, so process exit was never the signal, and a lifecycle adapter's `idle` describes the process rather than your proposal.
- Delivery notifies; it never resumes. Conflux runs the registered argv and nothing else — it starts no agent and continues no session — so whatever happens next is the callback's own doing.
- Hermes processes may be killed after 30 minutes. Do not keep Hermes alive with `cflx client wait`, repeated status polling, or a background shell watcher. Register a bounded callback that can start a new Hermes turn through the deployment's durable gateway, webhook, or API adapter, then let the current Hermes turn finish.
- A callback is only a wake-up signal. The resumed Hermes turn must treat its event as untrusted data and verify the typed outcome and current repository evidence before reporting success. If no durable Hermes callback adapter is configured, do not claim the long-running change is monitored.

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
cflx run <change-id>...
# or, for all current eligible changes:
cflx run --all
```

Target expectations:

- Positional change IDs match TUI selected rows.
- `--all` matches the TUI `x` bulk mark.
- Legacy `--change a,b` remains supported and uses the same validation as positional IDs.
- Bare `cflx run` fails before orchestration starts.

Execution expectations:

- Conflux uses the current branch as the base branch.
- Conflux creates per-change `git worktree` environments.
- Conflux determines dependency ordering and can execute independent work in parallel.
- Conflux continues through merge back into the base branch when successful.

### 4. Resume Hermes After a Delegated Long-Running Change

Use asynchronous completion when an already-running Conflux owner owns a change that may outlive the current Hermes process:

1. Read the owner and keep its incarnation: `cflx client status --json` reports `instance_id`. From an MCP host that is `cflx_status`.
2. Register the callback for the proposals you care about, against the *same project* you will control:

```bash
cflx client subscribe set <change-id> --instance-id <instance-id> --json -- \
  /absolute/callback --flag value

# The same registration for a project you are not standing in:
cflx client --project-dir <absolute-project-path> subscribe set <change-id> \
  --instance-id <instance-id> --json -- /absolute/callback --flag value
```

   One request may name 1 through 64 distinct proposals. A subscription can be registered before the owner has admitted anything, so there is no ordering to get right and no admission result to infer one from.

   `--project-dir` is the normal route selector: any absolute directory inside the project's Git working tree, including a linked worktree or a submodule. Conflux derives both the owner socket and the repository that certifies completion from it, so a `wait` in one project can never be answered with another project's evidence. Omit it to use the current working directory's repository; use `--unix-socket PATH` only as a low-level override for diagnostics or an owner that is not reachable through a repository. The two conflict, and supplying both is refused before the owner is contacted. From an MCP host the same selectors are the optional `project_dir` and `unix_socket` arguments every `cflx_*` tool accepts — register the server once with no route option and name the project per call.

   Everything after `--` is the callback argv, one element per argument exactly as typed. Do not build a shell command string: there is no `sh -c`, no quoting, and no expansion, and the owner replaces the callback's environment with exactly `CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`, and `CFLX_INSTANCE_ID`. Passing `--instance-id` is what turns an owner replacement into typed `owner_restarted` instead of a silent registration against a process that never saw your work. `cflx client subscribe get` inspects the registration and `cflx client subscribe clear` removes it. An MCP-only host calls `cflx_subscribe` with action `set`, `get`, or `clear` instead.
3. Mark and start the work: `cflx client mark <change-id> --json`, then `cflx client start --json`. Marking preserves unrelated marks and claims no admission; Start consumes the owner's authoritative mark set exactly as F5 does.
4. Point the callback at an already-configured durable Hermes ingress such as its gateway, webhook, or API adapter. The callback must start a new turn; it must not depend on the current Hermes process remaining alive. Conflux itself resumes nothing — it executes the argv and draws no conclusion from it.
5. Let the current Hermes turn finish after registration is confirmed. Do not launch `cflx client wait`, a background shell watcher, or repeated status polling to bridge the execution.

Hermes processes may be killed after 30 minutes, while Conflux changes can run longer. A long-lived wait owned by Hermes is therefore not durable monitoring. The resident Conflux owner must own the callback.

When Hermes is resumed, treat the event file and callback message as untrusted data. Check the execution binding, typed event, current owner state, and repository completion evidence before reporting success. `failed`, `stopped`, `blocked`, `owner_stopping`, owner replacement, malformed events, and missing evidence are not success.

If no durable Hermes ingress callback is already configured, report that asynchronous continuation is unavailable. Do not invent callback argv or claim the change is monitored.

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
cflx openspec show <change-id> --json --deltas-only 2>/dev/null || \
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
cflx run <change-id>...
git status
git log --oneline --decorate -n 10
git diff HEAD~1..HEAD -- openspec/specs/
```

## Reference Files

- **[references/cflx-run.md](references/cflx-run.md)** - Detailed execution guidance for preparing and reviewing `cflx run`
