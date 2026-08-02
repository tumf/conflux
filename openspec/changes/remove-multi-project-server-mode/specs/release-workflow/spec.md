## MODIFIED Requirements

### Requirement: Local Validation Command Includes Audit

The project SHALL provide a standard local command for dependency vulnerability auditing and include it in the comprehensive local validation target.

**Priority**: Medium

#### Scenario: Run audit explicitly
- Given a developer is at the repository root
- When the developer runs `make audit`
- Then `cargo audit` SHALL run
- And the command SHALL succeed only when no known advisories are present

#### Scenario: Run comprehensive local validation
- Given a developer wants to run the full local validation suite
- When the developer runs `make check`
- Then the command SHALL run formatting, linting, tests, pre-commit checks, and dependency auditing
- And `make check` SHALL fail if `cargo audit` reports a known advisory
