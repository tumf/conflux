## REMOVED Requirements

### Requirement: git/sync must only run reconciliation when needed before push

Removed with the server Git-sync API.

#### Scenario: No server reconciliation

**Given**: The retained router
**When**: Git-sync operations are enumerated
**Then**: No server reconciliation operation exists

### Requirement: git/sync must only run reconciliation when needed before push (pre-pull vs post-pull 比較版)

Removed with the server Git-sync API.

#### Scenario: No server pre-pull comparison

**Given**: The retained router
**When**: Git-sync operations are enumerated
**Then**: No server pre-pull comparison exists
