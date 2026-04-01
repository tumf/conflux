## MODIFIED Requirements

### Requirement: Dashboard Session Restoration on Reload

The dashboard frontend SHALL restore the selected project, active proposal session, and best-effort file-browse selection from the `ui_state` field in the FullState payload after a browser reload. When a persisted change or worktree browse target is restored successfully, the dashboard SHALL also restore enough tab state to surface the restored selection in the visible UI. If a persisted browse target no longer exists, the dashboard SHALL clear only the stale browse-related state and continue loading normally.

#### Scenario: Project selection restored

- **GIVEN** `ui_state` contains `selected_project_id=proj-1` and `proj-1` exists in the project list
- **WHEN** the dashboard receives the initial FullState message after reload
- **THEN** `proj-1` is automatically selected as the active project

#### Scenario: Proposal session restored

- **GIVEN** `ui_state` contains `active_proposal_session_id=ps-abc` and session `ps-abc` is active for the selected project
- **WHEN** the dashboard receives the proposal session list after reload
- **THEN** session `ps-abc` is automatically selected as the active proposal session tab

#### Scenario: Change selection restored

- **GIVEN** `ui_state` contains a persisted file-browse context that references change `change-a` for existing project `proj-1`
- **AND** the dashboard reloads while `change-a` is still available for that project
- **WHEN** the dashboard completes its initial restoration flow
- **THEN** the file-browse context is restored to `change-a`
- **AND** the Changes center tab and Files pane are selected so the restored change is visible

#### Scenario: Worktree selection restored

- **GIVEN** `ui_state` contains a persisted file-browse context that references worktree branch `feature-x` for existing project `proj-1`
- **AND** the dashboard reloads while worktree `feature-x` still exists for that project
- **WHEN** the dashboard completes its initial restoration flow
- **THEN** the file-browse context is restored to worktree `feature-x`
- **AND** the Worktrees center tab and Files pane are selected so the restored worktree is visible

#### Scenario: Stale browse reference cleaned up

- **GIVEN** `ui_state` contains a persisted file-browse context that references a change or worktree that no longer exists for the selected project
- **WHEN** the dashboard validates the persisted browse target during reload restoration
- **THEN** the dashboard calls the UI-state delete endpoint for the stale browse-related key or keys
- **AND** dashboard startup continues with the selected project still available

#### Scenario: Stale proposal session reference cleaned up

- **GIVEN** `ui_state` contains `active_proposal_session_id=ps-old` but session `ps-old` no longer exists
- **WHEN** the dashboard fetches the session list and `ps-old` is not found
- **THEN** the dashboard calls `DELETE /api/v1/ui-state/active_proposal_session_id` to clean up the stale reference
