---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/openspec_cmd.rs
  - src/openspec.rs
  - src/dependency_targets.rs
  - openspec/specs/cli/spec.md
  - openspec/specs/proposal-metadata/spec.md
---

# Add dependency status to openspec list

**Change Type**: implementation

## Problem / Context

`cflx openspec list` currently shows active OpenSpec changes with title, task progress, and path, but it does not expose proposal dependencies. Operators cannot quickly tell whether a listed change is waiting on an unfinished dependency, already unblocked by an archived dependency, running behind an in-flight dependency, or referencing a missing dependency.

Dependencies are already available from `proposal.md` metadata and body parsing, and repository-visible dependency classification already exists for queued, in-flight, archived, and missing targets. The list output should reuse those workspace-derived facts instead of relying on external logs or hidden state.

## Proposed Solution

Add dependency status rendering to the human-readable `cflx openspec list` active-change output.

For each active change that declares dependencies, render a `Dependencies:` line containing each dependency as `<change-id> [<status>]`.

Display statuses SHALL be:

- `done` for archived dependencies
- `running` for dependencies present in `.conflux-inflight`
- `pending` for active change dependencies that are not archived or in-flight
- `missing` for dependency ids not found in active changes, in-flight markers, or archive entries

Dependencies SHALL be parsed using the existing proposal metadata behavior: frontmatter `dependencies` takes precedence, and body `## Dependencies` remains the fallback when frontmatter does not define dependencies.

## Acceptance Criteria

- `cflx openspec list` shows a `Dependencies:` line for active changes that declare dependencies.
- Each rendered dependency includes a status label that makes unfinished dependencies visible.
- Archived dependency references are shown as `done`.
- Active dependency references are shown as `pending` unless they are currently in-flight.
- In-flight dependency references are shown as `running`.
- Unresolvable dependency references are shown as `missing`.
- Changes without dependencies do not show an empty `Dependencies:` line.
- `cflx openspec list --specs` output remains unchanged.
- Dependency status classification is derived only from workspace-local file state and git-visible OpenSpec archive state.

## Explicit Completion Conditions

This proposal is complete when:

- `src/openspec_cmd.rs` list-change data includes parsed dependencies and display statuses.
- Human-readable `cmd_list(false)` output renders dependency statuses for dependent changes.
- Existing dependency parsing in `src/openspec.rs` is reused or kept semantically consistent for frontmatter and body fallback behavior.
- Dependency target classification is reused from or aligned with `src/dependency_targets.rs` so queued, in-flight, archived, and missing semantics do not diverge.
- Tests cover pending, running, done, missing, no-dependencies, and `--specs` unchanged behavior.
- `cargo test openspec_cmd --lib` passes.
- `cargo test dependency_targets --lib` passes if shared classification code is touched.

## Out of Scope

- Adding a `--json` option to `cflx openspec list`.
- Changing dependency analysis prompts or scheduler behavior.
- Changing TUI dependency rendering.
- Treating external logs, cached metrics, or non-workspace state as authoritative dependency status inputs.
