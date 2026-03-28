# Change: Fix server mode dashboard project display

**Change Type**: implementation

## Problem/Context

In server mode, the dashboard shows newly added projects as `/` instead of displaying the repository and branch names. The backend WebSocket and `/projects/state` payloads currently serialize Rust `RemoteProject` objects with `id`, `name`, and `changes`, while the dashboard expects project records with separate `repo` and `branch` fields plus status metadata. This contract mismatch causes `dashboard/src/components/ProjectCard.tsx` and the selected-project header in `dashboard/src/App.tsx` to render empty values around the `/` separator.

## Proposed Solution

Align the server-mode project payload with the dashboard's display needs by returning explicit repository name and branch fields, along with project status metadata, in the server state snapshot used by REST and WebSocket updates.

Specifically:
- Introduce a server/dashboard-facing project snapshot shape that includes `id`, `repo`, `branch`, `status`, `is_busy`, and `error`
- Derive `repo` from the registered `remote_url` and preserve the configured `branch`
- Use the same normalized project shape in both `GET /api/v1/projects/state` and WebSocket `full_state` updates
- Preserve a stable, non-empty display even for unusual remote URLs by falling back to the best available repo label
- Update dashboard types to match the actual server contract used in server mode

## Acceptance Criteria

1. After adding a project in server mode, the dashboard project card displays `repo / branch` instead of `/`
2. The selected project header in the dashboard displays the same `repo / branch` values
3. REST state fetches and WebSocket `full_state` updates provide the same project field shape for dashboard rendering
4. For standard git remotes ending in `.git`, the displayed repo omits the `.git` suffix
5. If repo extraction is imperfect or the remote URL is unusual, the dashboard still avoids a bare `/` display
6. Existing server mode project status rendering continues to work with the updated payload

## Out of Scope

- TUI project grouping/display changes
- Changing project IDs or registry persistence format
- Broader redesign of dashboard project cards beyond correcting repo/branch display
