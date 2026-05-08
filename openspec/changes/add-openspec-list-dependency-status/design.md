# Dependency status rendering design

## Context

The requested behavior is a CLI presentation improvement for `cflx openspec list`. It should surface existing dependency metadata and repository-visible dependency state without changing orchestration behavior.

## Data flow

1. `OpenSpecManager::list_changes()` enumerates active change directories as it does today.
2. `OpenSpecManager::get_change_info()` reads each change's `proposal.md` through the existing proposal metadata parser.
3. The list path classifies each dependency using workspace-local evidence:
   - archived change ids under `openspec/changes/archive`, with date prefixes stripped
   - in-flight ids from `.conflux-inflight`
   - active change ids under `openspec/changes`
4. `cmd_list(false)` renders dependencies only when the change declares at least one dependency.

## Display mapping

The internal dependency target concepts remain `queued`, `in-flight`, `archived`, and `missing`, but list output uses operator-facing completion labels:

| Internal classification | CLI list label |
| --- | --- |
| `Archived` | `done` |
| `InFlight` | `running` |
| `Queued` | `pending` |
| `Missing` | `missing` |

This mapping makes the unfinished states (`pending`, `running`, `missing`) visible while still keeping `done` obvious for completed dependencies.

## Constitution alignment

The status line must not depend on external logs, metrics, or caches. `.conflux-inflight`, active change directories, and archive directories are workspace-local file state. Archive date-prefix stripping follows the repository-visible archive naming convention.

## Non-goals

This design does not add JSON output, does not alter scheduler dependency analysis, and does not change TUI rendering.
