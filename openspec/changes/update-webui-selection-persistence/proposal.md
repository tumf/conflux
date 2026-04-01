---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/specs/dashboard-api/spec.md
  - openspec/specs/server-persistence/spec.md
  - dashboard/src/App.tsx
  - dashboard/src/store/useAppStore.ts
---

# Change: Persist server mode WebUI selection state across reloads

**Change Type**: implementation

## Problem / Context

The server mode dashboard already persists `selected_project_id` and `active_proposal_session_id` in the server-backed `ui_state` store and restores them after reload. However, the current browser state for selected change and selected worktree only lives in the in-memory `fileBrowseContext` state. After a browser reload, users often return to the correct project but lose the concrete change/worktree context they were inspecting.

This creates an inconsistent restore experience for the WebUI, especially because the Files pane and tab layout depend on the selected browse target. The requested behavior is best-effort persistence: restore what still exists, silently clear stale references, and avoid blocking normal dashboard startup.

## Proposed Solution

- Extend dashboard UI-state persistence so the selected file browse target can be restored after reload.
- Persist enough auxiliary tab state to make restored selection visible without requiring the user to re-open the relevant pane manually.
- Define best-effort restoration rules that validate persisted references against the current project and worktree/session data before applying them.
- Self-heal stale UI state by deleting invalid persisted references when the referenced project/change/worktree no longer exists.

## Acceptance Criteria

1. If a persisted `selected_project_id` still exists, the dashboard restores that project after reload.
2. If a persisted file-browse selection references an existing change, the dashboard restores that change selection after reload and shows the Files view with the Changes center tab selected.
3. If a persisted file-browse selection references an existing worktree, the dashboard restores that worktree selection after reload and shows the Files view with the Worktrees center tab selected.
4. If a persisted browse target is stale, the dashboard clears only the stale browse state and continues loading normally.
5. Persisted UI-state restoration remains best-effort and must not break normal dashboard startup when referenced entities are missing.

## Out of Scope

- Persisting transient dialog open/closed state
- Persisting arbitrary file tree expansion or scroll position inside the file browser
- Changing proposal-session message restoration behavior beyond existing project/session restore flows

## Impact

- Affected specs: `dashboard-api`, `server-persistence`
- Affected code: `dashboard/src/App.tsx`, `dashboard/src/store/useAppStore.ts`, UI-state persistence usage in the dashboard frontend
