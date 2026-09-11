## ADDED Requirements

### Requirement: Centralize lifecycle status predicates

The repository MUST implement centralize lifecycle status predicates within the declared boundary while preserving existing external contracts.

#### Scenario: Focused refactoring preserves behavior

**Given**: the current repository behavior and focused regression fixtures
**When**: the implementation is refactored according to this change
**Then**: `pushed` contributes to completed totals in every Web snapshot construction path.
**And**: the declared focused verification passes without external credentials or deployment

#### Scenario: Unrelated behavior remains unchanged

**Given**: code and contracts outside the declared change boundary
**When**: this change is implemented and archived
**Then**: public formats and unrelated lifecycle behavior remain unchanged
