# Design: JSON task-file support

## Decision

Treat a change's task artifact as one format-discriminated repository file, not as a Markdown file with scattered JSON fallbacks. Existing Markdown semantics remain authoritative where format parity matters.

## Authoritative filenames and resolution modes

A selected change entry may contain exactly one of `tasks.md` or `tasks.json`. Both files in the selected entry are ambiguous and fail closed. An invalid or ambiguous higher-priority entry is not hidden by a lower-priority entry.

The implementation preserves four existing resolution modes rather than replacing them with one order:

1. **progress**: worktree active, worktree archive, base archive, base active;
2. **active-only**: worktree active, base active;
3. **archived**: worktree archive, worktree active, base archive;
4. **follow-up mutation/cleanup**: worktree active, then worktree archive, with no base-tree fallback.

Each candidate entry checks both supported filenames before moving to the next candidate. Read and mutation callers retain their current mode. In particular, Acceptance follow-up mutation never writes the base tree.

## JSON schema

`tasks.json` is one UTF-8 JSON object:

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
      "verification": {
        "kind": "unit",
        "command": "cargo test --lib"
      }
    }
  ],
  "narrative": {
    "future_work": [],
    "out_of_scope": [],
    "notes": [],
    "final_validation": "cflx openspec validate add-json-task-files --archive-gate"
  },
  "acceptance_follow_up": {
    "attempt": 2,
    "findings": [
      {
        "identity": "stable-finding-id",
        "text": "Repair the rejected behavior",
        "finding": null,
        "remediation_claimed": false,
        "evidence": []
      }
    ],
    "external_blockers": [
      {
        "identity": "external-id",
        "text": "Resolve the external prerequisite",
        "evidence": []
      }
    ]
  }
}
```

### Root and ordinary tasks

- `schema_version` is required and equals `1`.
- `tasks` is required and is an array containing only active tasks.
- Each task has a non-empty unique `id`, non-empty `title`, status `pending`, `in_progress`, or `completed`, and section `implementation` or `specification`.
- Unknown section or status values fail closed. Narrative and runtime follow-up content are invalid inside `tasks`.
- `verification.kind`, when present, is one of `unit`, `integration`, `e2e`, `manual`, `benchmark`, or `not-testable`.
- `verification_id` and `verification` are optional at schema level. Native strict validation requires `verification_id` for every active implementation/hybrid task when the proposal uses the role-model verification contract, exactly as for Markdown. `verification.command` is descriptive task evidence and does not replace the authoritative frontmatter `verifications[].evidence` and `.rerun` command sources.
- `narrative` is optional and never contributes to progress. `final_validation` is prose, not a task/status; strict validation rejects self-referential archive validation represented as an ordinary task.
- Unknown object fields are retained across Conflux-owned writes.

Only `completed` ordinary tasks contribute completed ordinary-task count. An empty ordinary-task list is not archive- or merge-complete.

### Acceptance follow-up

`acceptance_follow_up` is optional. When present:

- `attempt` is a positive integer;
- `findings` and `external_blockers` are arrays;
- each finding has non-empty `identity` and `text`, optional validated structured `finding` payload, boolean `remediation_claimed`, and string-array `evidence`;
- structured `finding`, when present, is runtime-owned and must validate through the existing repository-finding validator;
- legacy or synthesized findings use `text` with `finding: null`, preserving their actionable message;
- each external blocker has non-empty `identity` and `text`, plus string-array `evidence`;
- absent follow-up or empty `findings` plus empty `external_blockers` reads as no current follow-up;
- the existing synthesized fallback finding is emitted when Acceptance supplies no internal or external finding, matching Markdown behavior.

For progress parity, every internal finding is a virtual task: it contributes one to total and contributes one to completed only when `remediation_claimed` is true. External blockers retain current Markdown behavior: they are not checkbox tasks and do not alter progress counts; Acceptance state blocks them separately. Thus completed implementation tasks plus an unclaimed internal finding cannot authorize archive or merge.

JSON unknown fields are retained in place, so JSON mutation does not create recovered-note blocks. `FollowUpRecovery` reports zero recovered blocks for valid JSON. Invalid owned follow-up fields fail closed instead of being relocated.

## Shared API boundary

The task module owns:

- `TaskFileFormat` and resolved `TaskFile` path;
- the four resolution modes and candidate-entry ambiguity checks;
- progress parsing and semantic task projection;
- proposal validation projection;
- runtime Acceptance follow-up read, atomic replace, and cleanup;
- recovery-task append/insert required by rejection paths;
- selected path and format-aware diagnostic rendering.

Existing Markdown parsing, diagnostics, section classification, and follow-up semantics remain one format implementation without message changes. JSON validation emits semantic task records directly; it does not synthesize Markdown. JSON diagnostics use `tasks.json:<JSON Pointer>`, for example `tasks.json:/tasks/3/verification_id`.

## Archive and Git-diff recognition

Archive validation cannot infer a deleted active filename from filesystem existence. Git-diff verification accepts either exact deletion path `openspec/changes/<id>/tasks.md` or `tasks.json`, then requires the corresponding same basename at the exact archived entry. It rejects cross-format moves, both basenames, nested archive layouts, unrelated change IDs, or a missing add/delete pair. Filesystem archive predicates and path constructors likewise accept an explicit task-file format instead of silently constructing `tasks.md`.

## Failure behavior

Both files, invalid UTF-8/JSON, unsupported version, missing task array, duplicate/blank IDs, blank titles, unknown sections/statuses, invalid follow-up payload, and atomic mutation failure are typed task-file errors. They fail closed and never produce false `0/0`, completion, archive readiness, or merge authorization.

Read-only TUI/Web refresh keeps existing last-known progress on these errors. Apply, Acceptance mutation, archive, and merge authorization stop.

## Atomic mutation and ownership

JSON updates use a same-directory temporary file, flush, and rename, matching the existing Markdown atomic writer. Conflux owns only `acceptance_follow_up` and rejection-resume recovery-task insertion. AI Apply agents own ordinary task-status transitions, as they own Markdown checkbox transitions today. Conflux does not autonomously rewrite ordinary JSON statuses. Unknown fields at root and nested object levels survive Conflux-owned writes.

## Native OpenSpec boundary

`cflx openspec` list, show, validate, and archive are native Rust paths in this repository. Task-file validation, progress, and archive checks remain native and use the shared abstraction. Conflux does not delegate JSON task semantics to the upstream `openspec` CLI. Any unrelated external OpenSpec interoperability remains out of scope and cannot be used as completion evidence for JSON-only changes.

## Compatibility

`tasks.md` remains the default artifact produced by current proposal tooling and retains current syntax, diagnostics, progress, mutation, and completion behavior. This change adds `tasks.json` consumption and safe mutation; it does not migrate repositories, support mixed formats in one selected entry, or change upstream OpenSpec.
