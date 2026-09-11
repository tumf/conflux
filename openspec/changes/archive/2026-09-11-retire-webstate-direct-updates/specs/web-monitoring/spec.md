## ADDED Requirements

### Requirement: Retire direct WebState production updates

The repository MUST implement retire direct webstate production updates within the declared boundary while preserving existing external contracts.

#### Scenario: Focused refactoring preserves behavior

**Given**: the current repository behavior and focused regression fixtures
**When**: the implementation is refactored according to this change
**Then**: No production module can mutate `WebState` through the retired direct update API.
**And**: the declared focused verification passes without external credentials or deployment

#### Scenario: Unrelated behavior remains unchanged

**Given**: code and contracts outside the declared change boundary
**When**: this change is implemented and archived
**Then**: public formats and unrelated lifecycle behavior remain unchanged
