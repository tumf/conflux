# Task files

A change entry owns exactly **one** task artifact. Conflux accepts two
representations of it:

| Path | Representation | Status |
| --- | --- | --- |
| `openspec/changes/<id>/tasks.md` | Markdown checkboxes | Default; what proposal tooling produces |
| `openspec/changes/<id>/tasks.json` | Versioned structured tasks (`schema_version: 1`) | Structured alternative |

Both files in the same entry is an **ambiguity error**. Progress, proposal
validation, Apply, Acceptance, archive authorization, and final-merge
authorization all fail closed until exactly one remains; neither file wins by
precedence. Nothing migrates a change between formats, and one change never
splits its tasks across both.

Agent prompts name the resolved artifact as `tasks_path`, so an agent never has
to guess a filename.

## Choosing a format

`tasks.md` stays the default. Choose `tasks.json` when deterministic machine
updates matter more than prose — its status field is a closed enum, so a task's
completion cannot be expressed ambiguously.

## `tasks.json` v1

```json
{
  "schema_version": 1,
  "tasks": [
    {
      "id": "parser",
      "title": "Implement the task-file parser",
      "status": "pending",
      "section": "implementation",
      "verification_id": "json-task-file-tests",
      "verification": { "kind": "unit", "command": "cargo test --lib" }
    }
  ],
  "narrative": {
    "future_work": [],
    "out_of_scope": [],
    "notes": [],
    "final_validation": "cflx openspec validate <id> --archive-gate"
  },
  "acceptance_follow_up": {
    "attempt": 2,
    "findings": [
      {
        "identity": "repository|id|F1",
        "text": "Regression coverage is missing",
        "finding": null,
        "remediation_claimed": false,
        "evidence": []
      }
    ],
    "external_blockers": []
  }
}
```

### Active tasks

- `schema_version` is required and equals `1`.
- `tasks` is required and holds only **active** tasks.
- Each task has a unique non-empty `id`, a non-empty `title`, a `status` of
  `pending` / `in_progress` / `completed`, and a `section` of `implementation` /
  `specification`.
- Only `completed` counts as complete. An empty `tasks` array is **not**
  archive- or merge-complete.
- `verification.kind`, when present, is one of `unit`, `integration`, `e2e`,
  `manual`, `benchmark`, `not-testable`. `verification.command` is descriptive
  task evidence; the authoritative command sources remain the proposal
  frontmatter's `verifications[].evidence` and `.rerun`.
- `verification_id` is optional at schema level. Native strict validation
  requires it for every active task of a behavior-changing proposal that uses
  the role-model verification contract — the same rule Markdown tasks follow.

### Narrative

`narrative` is optional and never contributes to progress. `final_validation`
is prose: representing archive validation as an ordinary task is rejected, just
as a Final Validation checkbox is in Markdown.

### Runtime-owned acceptance follow-up

`acceptance_follow_up` is written by Conflux, not by agents.

- `attempt` is a positive integer.
- Each entry in `findings` has a non-empty `identity` and `text`, an optional
  validated structured `finding` payload, a boolean `remediation_claimed`, and a
  string array `evidence`.
- Every internal finding is a **virtual task-gate item**: it adds one to the
  total and one to completed only when `remediation_claimed` is `true`. So
  completed implementation tasks plus one unclaimed finding cannot authorize
  archive or merge.
- `external_blockers` are not tasks and never change progress counts.

An Apply agent may set `remediation_claimed` and append `evidence` for an
existing finding. It may never add, remove, reword, re-identify, or reorder a
finding, and never edit a `finding` payload. Acceptance PASS removes only the
runtime-owned follow-up state.

### Ownership and safe mutation

| Field | Owner |
| --- | --- |
| `tasks[].status` | Apply agent |
| `acceptance_follow_up` | Conflux runtime (agents may claim remediation and add evidence) |
| Rejection-recovery task insertion | Conflux runtime |
| Everything else, including unknown fields | Whoever wrote it — preserved untouched |

Conflux-owned writes are atomic (same-directory temporary file plus rename) and
preserve unknown additive fields at every object level, so an extension you add
survives a follow-up rewrite.

### Diagnostics

Every JSON failure is reported as `tasks.json:<JSON Pointer>`, for example:

```
add-json-task-files: tasks.json:/tasks/3/verification_id: active implementation task must reference a change-blocking verification
```

Malformed JSON, an unsupported `schema_version`, a missing `tasks` array, a
duplicate or blank `id`, a blank `title`, an unknown `section` or `status`, an
invalid follow-up payload, and a failed atomic write are all typed errors. They
fail closed: none of them can produce a false `0/0`, completion, archive
readiness, or merge authorization. Read-only TUI and Web refresh keep the
last-known progress on such an error instead of showing a fabricated one.

## Archive evidence

An archive moves the task artifact from the active entry to the archived entry
**without changing its basename**. A commit that deletes
`openspec/changes/<id>/tasks.json` must add `tasks.json` at the valid archived
entry (`archive/<id>/` or `archive/YYYY-MM-DD-<id>/`). Cross-format moves and
two competing basenames are refused; a nested archive layout or an unrelated
change ID is not this change's evidence.

## Compatibility

`tasks.md` keeps its current syntax, diagnostics, progress semantics, mutation
behavior, and completion rules exactly as before. This support adds `tasks.json`
consumption and safe mutation; it does not migrate repositories, support mixed
formats in one entry, or change upstream OpenSpec.
