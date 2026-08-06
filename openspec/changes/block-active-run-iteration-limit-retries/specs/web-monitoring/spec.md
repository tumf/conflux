## MODIFIED Requirements

### Requirement: API v2 browser operator console

The embedded web-monitoring interface MUST use `/api/v2` as its only production data, observation, error, and mutation contract. It MUST discover capabilities, read one coherent process snapshot, display process identity, and submit only advertised typed commands. Per-change controls MUST be rendered from the server-provided `actions` eligibility; display status, diagnostics, logs, and iteration counts MUST NOT independently authorize Retry. Production browser code MUST NOT call legacy `/api/*` or `/ws` routes.

#### Scenario: Console bootstraps from one process

**Given**: A cflx process serves web monitoring
**When**: A user opens the embedded console
**Then**: The browser reads `/api/v2/health`, capabilities, and state
**And**: The rendered mode, changes, totals, and process identity come from the coherent v2 response

#### Scenario: Production assets contain no legacy client route

**Given**: The packaged web assets
**When**: Their network targets are inspected
**Then**: They do not reference legacy `/api/*` resources or legacy `/ws`

#### Scenario: Server-blocked error row offers no Retry

**Given**: A change has `display_status=error`
**And**: Its authoritative `actions.retry_change` is blocked by `apply_iteration_limit_active`
**When**: The console renders the change
**Then**: It does not render an enabled or disabled Retry command control for that row
**And**: User interaction cannot submit `retry_change` for it
**And**: The console does not override the decision from the error status or iteration count

#### Scenario: Later allowed snapshot restores Retry

**Given**: A prior snapshot blocked Retry while an iteration-limited run was active
**When**: A later coherent snapshot reports `actions.retry_change.allowed=true`
**Then**: The console renders one Retry control for the change
**And**: Activating it submits exactly one typed `retry_change` command using the current revision
