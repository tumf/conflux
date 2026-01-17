## Context
In some cases, `openspec/changes/{change_id}` remains after an archive command. If the verification step still reports success, `ensure_archive_commit` fails and errors appear in the TUI. We need to detect this condition during verification and treat it as unarchived so the workflow retries or errors early.

## Goals / Non-Goals
- Goals:
  - Treat any existing `openspec/changes/{change_id}` as unarchived
  - Use consistent archive verification logic across parallel, TUI, and serial flows
- Non-Goals:
  - Change the archive command behavior itself
  - Change the archive destination layout

## Decisions
- Decision: Prioritize the absence of `openspec/changes/{change_id}` as the archive success condition in `verify_archive_completion`.
- Alternatives considered:
  - Keep prioritizing archive directory presence → rejected because unarchived changes slip through

## Risks / Trade-offs
- Keep the existing behavior that treats a missing change as successful even if an archive entry is missing for backwards compatibility.

## Migration Plan
- Update existing archive verification tests to cover the new condition.

## Open Questions
- None
