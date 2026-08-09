---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-architecture/spec.md
  - src/tui/render.rs
  - src/tui/runner.rs
  - src/tui/state.rs
  - src/tui/state/event_handlers/refresh.rs
  - src/vcs/git/commands/basic.rs
  - src/vcs/git/commands/status_policy.rs
verifications:
  - id: workspace-dirty-header-tests
    requirement: "The local TUI reports the captured repository root's staged, unstaged, and untracked dirty state in its header and refreshes that observation every five seconds without treating ignored files as dirty or using the display as workflow input"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust test output covering real temporary Git repositories, refresh-state adoption, dirty and clean header rendering, ignored-file exclusion, and failed-observation preservation"
    rerun: "cargo test --lib workspace_dirty_header && cargo test --lib refresh_workspace_dirty"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Show workspace dirty state in the TUI header

**Change Type**: implementation

## Premise / Context

- The local TUI header is rendered by `src/tui/render.rs::render_header` and currently shows process activity, worktree concurrency/backend facts, and the version.
- Local TUI auto-refresh already runs every five seconds from the repository root captured at startup in `src/tui/runner.rs`.
- `src/vcs/git/commands::has_uncommitted_changes` already provides the required read-only Git predicate using explicit untracked and ignored modes.
- That predicate counts staged, unstaged, and untracked changes while excluding ignored files and resisting user or repository `status.showUntrackedFiles` configuration.
- The dirty indicator is observability-only under the Constitution and must not become durable state or a workflow-control input.
- The Web operator console already exposes dirty state in its Worktrees view and is outside this TUI-focused change.

## Problem / Context

The TUI does not keep the operator informed when the repository workspace becomes dirty while Conflux is open. This is especially easy to miss during a long-running session because the header continues to show only orchestration and worktree runtime status. Operators must leave the TUI or infer the condition from later merge failures.

A one-time startup check would become stale, and adding a second timer or another dirty-state implementation would duplicate mechanisms already present in the TUI and Git layers. The display therefore needs to consume the existing five-second refresh cadence and the existing Git status policy.

## Proposed Solution

Add a process-local, tri-state workspace dirty observation to TUI presentation state and update it from the captured repository root on each existing five-second auto-refresh:

- call the existing `has_uncommitted_changes` helper once per refresh for the captured main repository root;
- publish a typed TUI refresh event only after a successful observation and adopt it into presentation state;
- represent an unobserved or failed initial check as unknown, a successful clean check as clean, and a successful non-empty porcelain result as dirty;
- preserve the last successful observation if a later Git status read fails, while emitting a bounded warning through the existing logging path;
- render a red bold `[dirty]` badge after `[workspaces:<max>:<backend>]` only when the latest successful observation is dirty;
- remove the badge after a later successful clean observation;
- keep the indicator out of reducer state, persisted state, command admission, queue selection, scheduler dispatch, resume routing, acceptance, archive, and merge decisions.

The existing refresh interval, captured repository root, read-only Git helper, and header layout remain authoritative. No new timer, dependency, configuration option, or Web/API field is introduced.

## Acceptance Criteria

1. A clean captured repository root renders no `[dirty]` badge in the local TUI header.
2. A staged change, unstaged change, or untracked file at the captured repository root renders a red bold `[dirty]` badge after the workspaces badge.
3. Ignored files alone do not render `[dirty]`, including when Git configuration would otherwise alter untracked-file reporting.
4. Dirty-to-clean and clean-to-dirty transitions become visible after the next existing five-second refresh without restarting the TUI.
5. Dirty observation always uses the repository root captured at TUI startup and does not follow a later process current-directory change.
6. If Git status cannot be observed, the TUI preserves its last successful dirty/clean observation, does not claim a failed check was clean, and emits a bounded warning rather than terminating refresh or orchestration.
7. Unknown initial state and known-clean state both omit the badge; unknown is not converted into workflow permission or cleanliness evidence.
8. The badge remains compatible with the existing right-aligned version area and does not replace Ready, Running, Stopping, modal, or workspaces header content.
9. Dirty presentation state does not change any reducer, command, queue, scheduler, resume, acceptance, archive, merge, or next-action result.
10. WebUI/API behavior and worktree-row dirty presentation remain unchanged.

## Explicit Completion Conditions

- `src/tui/runner.rs` observes dirty state on the existing `AUTO_REFRESH_INTERVAL_SECS` task using the captured `repo_root` and `crate::vcs::git::commands::has_uncommitted_changes`; no second polling task or duplicate Git-status parser exists.
- A typed TUI event and `AppState` presentation field carry successful dirty observations without adding the fact to the orchestration reducer or durable state.
- Failed status reads leave the previous successful value intact and produce no false clean update.
- `src/tui/render.rs::render_header` conditionally emits exactly `[dirty]` with warning styling after the existing workspaces badge.
- Real temporary-repository tests prove staged, unstaged, untracked, ignored-only, clean-after-dirty, explicit-root, and observation-failure behavior.
- Render tests prove dirty shows the badge, clean/unknown omit it, existing header content remains, and the version area still renders.
- The declared `workspace-dirty-header-tests` verification passes.

## Scope Rationale

Observation, presentation-state adoption, and header rendering form one independently verifiable TUI behavior. Splitting them would leave either unused state wiring or an indicator with no live data, so they must ship together.

## Out of Scope

- Adding a dirty badge to the WebUI header or changing `/api/v2` contracts.
- Showing dirty file counts, paths, staged/unstaged categories, branch divergence, or ignored files.
- Making the refresh interval configurable or adding filesystem watchers.
- Blocking execution, merge, resolve, archive, or shutdown because the badge is present.
- Changing existing worktree deletion safety or remote worktree dirty semantics.
- Persisting dirty observations across process restarts.

The tracked Rust hooks in `.pre-commit-config.yaml` are path-scoped and do not run for proposal-only commits. Requirement-specific focused tests therefore remain explicit implementation evidence rather than being delegated to this proposal commit's hooks.
