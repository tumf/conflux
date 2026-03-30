## Requirements

### Requirement: stats-overview-api-contract-test

The `/api/v1/stats/overview` endpoint response JSON structure must be validated by automated tests against the frontend `StatsOverview` TypeScript type to prevent runtime type mismatches.

#### Scenario: Rust API test validates response field structure

**Given**: A server with at least one recorded change event
**When**: `GET /api/v1/stats/overview` is called
**Then**: The response JSON contains `summary` (object with `success_count`, `failure_count`, `in_progress_count`, `average_duration_ms`), `recent_events` (array of objects with `project_id`, `change_id`, `operation`, `result`, `timestamp`), and `project_stats` (array of objects with `project_id`, `apply_success_rate`, `average_duration_ms`, `success_count`, `failure_count`, `in_progress_count`)

### Requirement: stats-overview-frontend-resilience-test

The `OverviewDashboard` component must render without crashing even when the API response is missing expected fields.

#### Scenario: Dashboard renders with complete StatsOverview response

**Given**: A mocked API returning a complete `StatsOverview` response
**When**: `OverviewDashboard` is rendered
**Then**: Summary cards, recent events list, and project stats are displayed without errors

#### Scenario: Dashboard renders with partial API response

**Given**: A mocked API returning a response where `recent_events` or `project_stats` is undefined
**When**: `OverviewDashboard` is rendered
**Then**: Fallback UI is shown (e.g., "No recent events", "No project stats") and no TypeError is thrown
