---
change_type: implementation
priority: high
dependencies: []
references:
  - src/server/api.rs
  - src/server/db.rs
  - dashboard/src/api/types.ts
  - dashboard/src/api/restClient.ts
  - dashboard/src/components/OverviewDashboard.tsx
---

# Add Stats Overview API Contract Tests

**Change Type**: implementation

## Problem/Context

The `/api/v1/stats/overview` endpoint response structure diverged from the frontend `StatsOverview` TypeScript type, causing a runtime `TypeError: Cannot read properties of undefined (reading 'length')` in the dashboard. The root cause was that:

1. The Rust `StatsOverviewResponse` returned a flat `{ success_count, failure_count, average_duration_ms }` structure.
2. The frontend `StatsOverview` type expected `{ summary, recent_events, project_stats }`.
3. No test validated the contract between backend JSON and frontend type expectations.

The backend response has been fixed to match the frontend type, but no cross-layer contract test exists to prevent future regressions.

## Proposed Solution

Add two layers of contract testing:

1. **Rust API test**: Validate that the `/api/v1/stats/overview` JSON response matches the exact field structure expected by the frontend `StatsOverview` type (nested `summary`, `recent_events[]`, `project_stats[]`).

2. **Frontend integration test**: Render `OverviewDashboard` with a mock API returning the expected response shape, and also test resilience when fields are missing/undefined (defensive rendering without crash).

## Acceptance Criteria

- A Rust test asserts that the `/api/v1/stats/overview` response JSON contains `summary.success_count`, `summary.failure_count`, `summary.in_progress_count`, `recent_events` (array), and `project_stats` (array).
- A frontend test renders `OverviewDashboard` with a mocked API response matching `StatsOverview` and verifies no crash occurs.
- A frontend test renders `OverviewDashboard` with a partial/malformed API response (missing `recent_events`, missing `project_stats`) and verifies graceful degradation (no crash, fallback UI shown).

## Out of Scope

- E2E browser tests with a real backend server.
- Schema generation or OpenAPI spec automation.
