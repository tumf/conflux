## REMOVED Requirements

### Requirement: Server mode dashboard project state must expose display-ready project identity

Removed with the server-mode dashboard.

#### Scenario: No server dashboard identity

**Given**: Packaged interfaces
**When**: dashboard surfaces are inspected
**Then**: No server-mode project state is rendered

### Requirement: Server-mode dashboard shows per-project sync state

Removed with the server-mode dashboard.

#### Scenario: No project sync dashboard

**Given**: Packaged interfaces
**When**: views are inspected
**Then**: No server project sync view exists

### Requirement: Dashboard worktree deletion shows row-level pending state

Removed with the standalone dashboard.

#### Scenario: No dashboard worktree deletion

**Given**: Packaged interfaces
**When**: actions are inspected
**Then**: No standalone dashboard deletion action exists
