# Design: Progress-aware acceptance retry

## Context

A FAIL is only useful when apply can change repository evidence. Repeating the same review with no repository progress wastes agent cycles and can end as an irreversible-looking terminal error. At the same time, stopping on the first repeated text is unsafe because findings can remain valid while a real partial fix is in progress.

## Decision

### Finding identity

The portable verdict continues to accept string findings. Runtime normalizes each finding into an identity equivalent to:

```text
scope + stable code + repository path + normalized message core
```

Structured findings may provide these components directly. Legacy strings use deterministic extraction and normalization. Comparison sorts and deduplicates the identity set; presentation order is not significant. Human-readable detail remains available separately.

Repository-fixable and external findings are classified per finding, not by concatenating the full set. Ambiguous findings default to repository-fixable for the initial retry so the runtime does not hide a potentially repairable defect.

### Semantic progress

A snapshot represents repository-visible implementation semantics between acceptance attempts. It includes tracked and untracked changes in source, tests, configuration, proposal/spec content, and substantive task content, whether committed or not.

Before comparison, runtime-managed `Current Acceptance Follow-up` content and legacy numbered acceptance follow-up sections are stripped from `tasks.md`. `APPLY_BLOCKED/marker.md`, acceptance attempt counters, external logs, and UI/history state are excluded. A changed HEAD alone is not progress if its only semantic change is excluded bookkeeping.

### Retry decision

1. Initial FAIL writes current follow-up and always permits one apply retry.
2. After apply, acceptance runs again.
3. If the finding identity set changed or semantic progress occurred, another retry remains eligible, bounded by the existing cycle ceiling.
4. If the identity set is unchanged and no semantic progress occurred, runtime writes a resumable stalled marker and stops before apply.
5. At cycle 10, runtime writes an `acceptance_cycle_limit_exhausted` stalled marker rather than emitting terminal Error.

### Follow-up ownership

Acceptance remains read-only. Runtime owns exactly one `## Current Acceptance Follow-up` section. Repository-fixable findings are unchecked tasks; external blockers are non-checkbox metadata with owner/evidence/next action. Replacing the section removes obsolete findings. A persistence error is reported but does not replace the primary acceptance verdict.

### Durable stalled state

The existing `openspec/changes/<id>/APPLY_BLOCKED/marker.md` is reused. Acceptance-generated markers identify their origin and include reason, retry count, normalized finding identities, current findings, external blockers, resumability, and next action.

Ordinary dispatch honors the marker. Explicit retry may consume only a resumable acceptance-generated marker, after surfacing its context; it must not clear unrelated apply-generated blockers. This makes restart behavior derivable from workspace files.

### Prompt context

The next acceptance receives current diff and latest normalized findings. Full attempt history is not injected. Latest raw output is included only when no finalized FAIL finding payload exists, such as CONTINUE or command diagnostics.

## Alternatives Rejected

- Commit hash comparison: runtime bookkeeping commits create false progress.
- Exact free-form text comparison: line numbers and wording drift create false differences.
- Immediate stall on first FAIL: denies apply any repair opportunity.
- Keeping terminal cycle-limit Error: destroys resumable workflow intent.
- New state database: violates workspace-local routing law.

## Migration

On the next FAIL, runtime replaces all legacy numbered acceptance follow-up sections with the current managed section. Existing `APPLY_BLOCKED` markers without acceptance origin keep their current semantics and are never auto-cleared by this change.
