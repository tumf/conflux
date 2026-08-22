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
| Artifact | Repository-relative path plus content digest |
| Clean state | Evidence-path-excluded cleanliness before *and* after the run |

Reuse is an all-fields conjunction. There is no partial score and no freshness
heuristic: the first binding that differs is the rerun reason, reported per
verification ID as a stable code (`binding_mismatch`, `worktree_dirty`,
`evidence_missing`, `evidence_agent_authored`, …) plus a detail line naming what
changed.

## Where it lives

`.cflx/verification-evidence/` inside the change worktree, holding one
`<verification-id>.json` envelope and one `<verification-id>.log` artifact per
verification. The directory carries a self-ignoring `.gitignore`, so nothing is
added to a commit and `.git/info/exclude` is not touched.

That directory is also the one difference from the bound commit that does *not*
count as dirty — otherwise writing the sidecar would invalidate the sidecar. Any
change outside it forces rerun.

Everything the decision depends on is inside the worktree. Deleting
`~/.local/state/cflx/**` cannot change a decision, and a restart re-derives the
same answer from the same files, as constitutional law 1 requires.

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
`0400` inside a `0700` runtime-owned directory, and one whose mode or `authority`
field says otherwise is refused as `evidence_agent_authored`.

A process running under the same UID can `chmod` past that, so this is default
mutation refusal rather than an integrity guarantee. What makes a forged envelope
harmless is that forging it buys nothing: every binding is still compared against
live Git state, so the only envelope that survives validation is one describing a
command that really did run against exactly this tree — and a rerun would have to
satisfy the same conditions anyway.

**Never hand-edit anything under the evidence directory.** Editing an envelope
only forces the rerun it was trying to avoid.

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
