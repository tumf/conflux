## ADDED Requirements

### Requirement: Extract shared parallel test support

The repository MUST implement extract shared parallel test support within the declared boundary while preserving existing external contracts.

#### Scenario: Focused refactoring preserves behavior

**Given**: the current repository behavior and focused regression fixtures
**When**: the implementation is refactored according to this change
**Then**: Shared workspace, config, Git setup, and failure helpers live in `support.rs`.
**And**: the declared focused verification passes without external credentials or deployment

#### Scenario: Unrelated behavior remains unchanged

**Given**: code and contracts outside the declared change boundary
**When**: this change is implemented and archived
**Then**: public formats and unrelated lifecycle behavior remain unchanged
