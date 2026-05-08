# Design: Add dependency status to openspec show

## Overview

The existing dependency-status implementation for `cflx openspec list` already provides the desired source of truth: proposal dependency parsing plus workspace-local classification through active changes, `.conflux-inflight`, and archived change IDs. This change extends the detail-oriented show path to reuse the same model instead of creating a second classification mechanism.

## Data Flow

1. `OpenSpecManager::show_change()` finds the requested change directory as it does today.
2. For normal non-deltas-only active changes, it parses `proposal.md` metadata through `crate::openspec::parse_proposal_metadata_from_file()`.
3. It classifies dependencies through `DependencyStatusContext::from_workspace(&self.root_dir)` and `statuses_for()`.
4. It stores the dependency IDs/statuses on `ShowInfo`.
5. Human-readable `cmd_show()` renders the same `<id> [label]` format used by list output.
6. JSON `cmd_show()` emits structured entries suitable for downstream automation.

## Status Semantics

Show output must match list output:

| Workspace evidence | Label |
| --- | --- |
| archived dependency target | `done` |
| dependency target listed in `.conflux-inflight` | `running` |
| active dependency target | `pending` |
| no active, in-flight, or archived target | `missing` |

## Deltas-only Behavior

`--deltas-only` is intentionally preserved as a spec-delta-focused view. It should not become a dependency inspection surface in this change.

## Constitution Alignment

Dependency status remains derived from workspace file state only. The proposal does not introduce out-of-worktree durable workflow state and therefore aligns with the workspace-local workflow state law in `openspec/CONSTITUTION.md`.
