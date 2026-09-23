# Bound Verification Evidence

Apply often runs exactly the repository-local command a proposal declares for
Acceptance. Repeating it during Acceptance costs real time, and the cheap ways to
avoid the repeat — a log line, a narrative "tests passed", an agent-authored
result field — are all ways to turn *no evidence* into PASS.

Conflux takes the other trade. A verification result may be reused only when a
runtime-written sidecar still proves, against live Git state, that the command
ran against exactly this tree. Everything else runs the command again.

## The two commands

```bash
cflx openspec verify my-change --plan          # What would Acceptance decide right now?
cflx openspec verify my-change                 # Run whatever cannot be reused
cflx openspec verify my-change --verification-id local-tests --json
```

`--plan` executes nothing. The default runs only the verifications with no
surviving evidence, so a second invocation against an unchanged tree is nearly
free and an invocation after any real change runs the command again.

Exit status is `0` when every selected verification is reused or freshly
captured, and `1` when at least one is neither. A failing command is reported
with its exit code and artifact path — that is a *command* result, not a verdict
about the change.

## What gets bound

The runtime executor starts the declared argv itself, waits for it, hashes what
it captured, and records:

| Binding | Source |
| --- | --- |
| Commit and tree | Full-length `HEAD` and `HEAD^{tree}` object IDs |
| Command | The exact argv array, never a shell string |
| Working directory | Normalized repository-relative (`.` for the root) |
| Automation file | The declared path's blob ID at that commit |
| Tool | Resolved executable path, its file digest, and its `--version` output |
| Result | Start/end timestamps and the exit code of the child it waited on |
| Artifact | Store-relative path plus content digest |
| Clean state | Full target cleanliness before *and* after the run, with no Conflux-owned path excluded |

Reuse is an all-fields conjunction. There is no partial score and no freshness
heuristic: the first binding that differs is the rerun reason, reported per
verification ID as a stable code (`binding_mismatch`, `worktree_dirty`,
`evidence_missing`, `evidence_agent_authored`, …) plus a detail line naming what
changed.

## Where it lives

Outside the target. Constitutional law 4 forbids Conflux from writing its own
runtime artifacts into a target repository or any managed worktree, and it is
explicit that hiding such a write with an ignore file or a cleanliness exception
does not make it permissible. So nothing lives in the worktree at all:

```text
<conflux-state-root>/acceptance/
  <project-slug>/
    <workspace-slug>/
      <change-id>/
        manifest.json          # the frozen execution manifest
        diagnostics.json       # how the last invocation ended (read by humans only)
        gates/
          <verification-id>.json   # the bound envelope
          <verification-id>.log    # the captured artifact
```

The root follows the ordinary `state_base_dir` → `XDG_STATE_HOME` →
`~/.local/state` precedence, with a Conflux-owned system temporary root only when
no persistent one can be derived. Project and workspace slugs come from the same
canonical-path algorithm the worktree and log paths use, so two worktrees of one
project never share a cache and one worktree's cache is stable across runs.

**It fails closed, never into the target.** A root that cannot be resolved,
created, or written — and a root that resolves *inside* the target repository or
a managed worktree — produces the typed
`acceptance_state_path_unavailable` runtime hold before any declared gate is
launched. There is no `.cflx` fallback, because a fallback is exactly the
pollution this arrangement removes.

**Cleanliness has no exceptions.** Acceptance evaluates the target with the
ordinary tracked/untracked porcelain status and no Conflux-owned path excluded.
The old evidence-directory exception existed so that writing the sidecar could
not invalidate the sidecar; with the sidecar out of the tree there is nothing
left for it to protect.

## Losing the cache costs work, never correctness

Everything under that directory is **cache and diagnostics**. It is never
workflow authority:

- A missing, unreadable, malformed, stale, or deliberately deleted record means
  exactly one thing: run the declared command again.
- It never implies PASS, never suppresses a command, and never selects the next
  workflow action — admission, retry, archive, merge, and next-action selection
  are all decided from repository and Git state.
- Deleting `~/.local/state/cflx/**` may discard reusable evidence, but it cannot
  modify the target worktree or make its repository state look cleaner than it
  is.

That is constitutional law 1 in practice: the next action chosen for the same
workspace contents does not depend on out-of-worktree state.

## Legacy `.cflx/verification-evidence`

Earlier releases wrote evidence into `.cflx/verification-evidence/` inside the
worktree, behind a self-ignoring `.gitignore`. The current runtime treats that
path as **someone else's file**:

- It probes only whether the path exists — including when self-ignored, because
  the probe is a filesystem check rather than a `git status` question.
- If it exists, Acceptance stops with the typed
  `acceptance_legacy_target_evidence` hold naming the path.
- It never opens, parses, migrates, rewrites, hides, or deletes the content.
- No other ignored path is evaluated. A build-output directory or a dependency
  cache is none of Conflux's business, and inspecting one would reintroduce the
  Conflux-specific status filtering the constitution forbids.

Removing the directory is an explicit repository-owner action:

```bash
rm -rf .cflx/verification-evidence
```

## Retry refusal is process-local

An Acceptance invocation whose deadline expires settles as a non-resumable hold.
While the owner process that recorded it is alive, an unchanged retry of that
exact input is refused as `unchanged_acceptance_input`, and the refusal names the
fingerprint components a repository change would have to move.

That refusal lives in memory and nowhere else. A restarted owner reconstructs no
hold: the first admission in the new lifetime — an explicit retry or automatic
mark settlement — gets one fresh bounded attempt with no repository mutation
required, while still-valid cached gate results keep it cheap. A durable record
that could refuse work after a restart would mean deleting out-of-worktree state
changed the next action, which is the thing law 1 rules out.

## What is eligible

Only declarations that are:

- `execution_class: repository-local` **and** `completion_role: change-blocking`;
- a plain argv — a declared command containing `|`, `&&`, `;`, redirection, or
  `$` substitution is reported as `declaration_shell_syntax`, because the runtime
  starts a process rather than a shell;
- slower than the reuse threshold. Commands measured under 60 seconds stay on the
  ordinary rerun path: they are cheaper to run than to justify. The threshold is
  a repository-tracked constant that the runtime measures itself — proposal prose
  and agent claims cannot raise it.

Deployed, credentialed, device, and observation-only verifications have no local
reuse decision at all. They are owned elsewhere by design and are absent from the
plan.

## The authority boundary

Only the runtime executor may write a reusable envelope. Records are written
`0400` inside a `0700` runtime-owned directory in the Conflux state root, and one
whose mode or `authority` field says otherwise is refused as
`evidence_agent_authored`.

A process running under the same UID can `chmod` past that, so this is default
mutation refusal rather than an integrity guarantee. What makes a forged envelope
harmless is that forging it buys nothing: every binding is still compared against
live Git state, so the only envelope that survives validation is one describing a
command that really did run against exactly this tree — and a rerun would have to
satisfy the same conditions anyway.

**Never hand-edit anything under the store.** Editing an envelope only forces the
rerun it was trying to avoid. Deleting the whole directory is always safe.

## What Acceptance sees

Before each acceptance invocation the runtime computes the plan from the
workspace and passes it in as context stating, per verification ID, `reused` or
`rerun` and why. The same lines go to the log.

The plan is observability plus context. It never decides whether Acceptance runs,
never suppresses a command, and never becomes a verdict:

- `reused` means that command passed against this commit — never that the change
  is acceptable, and never a substitute for review.
- `rerun` means no binding evidence survives. It is not a finding, and it implies
  neither PASS nor FAIL.
