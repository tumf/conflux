---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/tui-key-hints/spec.md
  - src/tui/state.rs
  - src/tui/render.rs
  - src/vcs/git/commands/basic.rs
verifications:
  - id: tui-eligibility-reason-tests
    requirement: The TUI distinguishes actual proposal-directory changes from other parallel-ineligibility reasons and renders truthful row badges
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output covering eligibility classification, archived proposal absence, dirty proposal content, and Changes-list rendering
    rerun: cargo test --lib tui::state && cargo test --lib tui::render
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: repository-quality-gates
    requirement: The Rust implementation remains formatted, lint-clean, and valid across the default test suite
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: successful make fmt, make lint, and make test results
    rerun: make fmt && make lint && make test
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Fix TUI Uncommitted Badge Classification

**Change Type**: implementation

## Problem / Context

The TUI currently derives the `UNCOMMITED` badge from the broad predicate `!change.is_parallel_eligible`. Parallel eligibility combines two distinct facts: whether the change proposal exists in the current `HEAD` tree and whether files under `openspec/changes/<change_id>/` are uncommitted or untracked.

This causes a clean managed worktree to be labeled `UNCOMMITED` when its change is no longer present under `openspec/changes/` in the base tree, including a failed merge row for a change that was already archived. The badge therefore claims a Git working-tree condition that does not exist. The label is also misspelled.

The workspace and Git observations already permitted by `openspec/CONSTITUTION.md` remain the authoritative inputs. The defect is loss of reason information between eligibility observation and TUI rendering, not missing durable state.

## Proposed Solution

Represent the reason a change is not parallel-eligible instead of collapsing every reason into one boolean display meaning. Preserve the existing eligibility guard used to prevent unsafe queueing, but let rendering distinguish actual uncommitted or untracked proposal files from proposal absence in `HEAD` and from any other non-dirty ineligibility reason.

Render `UNCOMMITTED` only when repository observation reports uncommitted or untracked files under that change's active proposal directory. Do not render that badge merely because the proposal is absent from `HEAD`, including archived changes with a retained managed worktree or failed merge state. Continue to show the independent `WT` badge whenever a managed worktree exists.

Keep key-hint, checkbox, row styling, and row-action suppression based on broad parallel eligibility. A clean proposal absent from `HEAD` remains grayed out, non-markable, and without queue affordances exactly as today; only the badge and operator-facing refusal reason distinguish it from genuinely dirty proposal content. This is one atomic scope because reason classification without renderer, warning, and exclusion-summary wiring would preserve false claims, while a renderer-only fix would continue inferring from the lossy boolean.

## Acceptance Criteria

- A queued or not-queued active change with uncommitted or untracked files under `openspec/changes/<change_id>/` is non-actionable and displays `UNCOMMITTED`.
- A clean change that is parallel-ineligible only because its proposal is absent from the current `HEAD` tree does not display `UNCOMMITTED`.
- An archived change or failed-merge row with a retained clean managed worktree may display `WT` but does not display `UNCOMMITTED` solely because the active proposal directory is absent.
- The existing parallel-execution safety rule remains intact: a change absent from `HEAD` or containing dirty proposal files remains grayed out, non-markable, without queue affordances, and not admitted to parallel queueing merely because its badge differs.
- Single-row toggle warnings and bulk-toggle exclusion summaries describe the observed reason truthfully: dirty proposal content retains a commit instruction, while clean proposal absence identifies that the change is not present in `HEAD` and cannot be queued in parallel mode.
- Changes-list rendering, toggle warnings, and bulk exclusion summaries consume one consistent eligibility-reason model in select and running layouts and shared operator-command paths.
- All user-visible instances and tests use the correctly spelled `UNCOMMITTED`; `UNCOMMITED` is removed from the active TUI contract.

## Explicit Completion Conditions

- TUI state carries repository-derived parallel-ineligibility reason information without introducing durable state outside the workspace.
- Eligibility calculation still rejects both proposals absent from `HEAD` and proposals with uncommitted or untracked files.
- Renderer badge decisions test the explicit dirty-proposal reason, while key-hint, checkbox, styling, actionability, and admission decisions continue to block every parallel-ineligible reason.
- Single-row and bulk-toggle refusal paths distinguish dirty proposal content from clean proposal absence and provide an actionable, truthful message for each.
- Repository-local regression tests separately cover a dirty active proposal, a clean proposal absent from `HEAD`, and an archived or failed-merge change retaining a clean worktree while displayed in a queueable status.
- Regression tests prove the dirty case displays `UNCOMMITTED`, the clean absent-proposal case does not, both remain non-actionable, and neither case weakens queue admission.
- `make fmt`, `make lint`, and `make test` pass.

## Out of Scope

- Changing merge conflict resolution, retry counts, worktree deletion, or ahead/behind reconciliation.
- Making archived changes queueable or changing the underlying parallel eligibility policy.
- Inspecting arbitrary files outside `openspec/changes/<change_id>/` for this badge.
- Adding persisted UI or workflow state outside the repository workspace.
