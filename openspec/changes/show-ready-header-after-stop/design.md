## Context

`AppExecutionMode` is an internal command-admission and control state. The header is a user-facing projection of current orchestration activity, while the status panel carries mode-specific actions.

Stopped mode needs distinct status controls and execution-mark admission rules even though F5 ultimately uses the same `start_marked()` dispatch as Select mode. That internal distinction does not require a separate header status. Once the scheduler and agents are no longer executing, the header-level fact is Ready.

## Goals

- Display `[Ready]` after terminal stop.
- Preserve stopped-mode resume controls and command routing.
- Keep overlays presentation-only and preserve every other header mapping.
- Remove canonical contradictions about hidden and gray stopped labels.

## Non-Goals

- Renaming internal execution modes or API tokens.
- Changing queue, mark, resume, cancellation, or reducer behavior.
- Redesigning the header or adding configuration.

## Decision: Header Projects Activity, Status Panel Projects Action

Use two existing presentation layers for separate operator questions:

| Operator question | Surface | Stopped-mode answer |
|---|---|---|
| Is orchestration executing now? | Header | `[Ready]` |
| What action continues this session? | Status panel | configured start key + `resume` |

This keeps the header vocabulary small and truthful without collapsing internal control state. `AppExecutionMode::Stopped` remains unchanged, and rendering remains a pure read.

## Header Mapping

Without an active modal:

| Internal execution mode | Header |
|---|---|
| `Select` | cyan `[Ready]` |
| `Running` | yellow `[Running]` or `[Running N]` |
| `Stopping` | yellow `[Stopping]` |
| `Stopped` | cyan `[Ready]` |
| `Error` | no status label |

Modal labels continue to take presentation precedence. Closing or replacing a modal reveals the header projection of the unchanged underlying execution mode.

## Verification Strategy

Use the existing Ratatui `TestBackend` buffer helpers:

- stopped render contains `[Ready]` in `Color::Cyan`, asserted through the existing `fg_at` buffer helper, and the configured resume hint;
- stopped render never contains `[Stopped]`;
- rendering does not mutate `AppExecutionMode::Stopped`;
- modal-free Error renders neither `[Ready]` nor `[Stopped]` and remains retry-owned;
- Running count, Stopping label, QR, and destructive confirmation behavior remain unchanged.

No browser or external environment is required.

## Risks and Mitigations

- **Ready is mistaken for fresh Select mode:** the status panel still says `resume`, and command routing still reads internal `Stopped`.
- **Overlay close restores the wrong mode:** rendering does not mutate execution state; existing modal tests pin this boundary.
- **API clients lose stopped state:** API and lifecycle projections are out of scope and continue reading the internal mode/token.
- **Spec remains contradictory:** modify all three canonical CLI requirements that currently define stopped header presentation.
