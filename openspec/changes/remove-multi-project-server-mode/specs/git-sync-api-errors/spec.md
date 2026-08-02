## REMOVED Requirements

### Requirement: git_sync API は内部エラーでパニックしない

Removed with the server Git-sync API.

#### Scenario: No server Git-sync API

**Given**: The retained router
**When**: routes are enumerated
**Then**: No server Git-sync endpoint exists
