## MODIFIED Requirements

### Requirement: REQ-REL-002 Pre-release Validation

The release and CI validation flow SHALL include dependency vulnerability auditing in addition to formatting, linting, and test checks.

**Priority**: High

#### Scenario: CI audit passes
- Given the GitHub Actions checks job is running
- And project dependencies have no known RustSec advisories
- When the validation steps execute
- Then `cargo audit` SHALL run as part of CI validation
- And the checks job SHALL continue to subsequent validation steps

#### Scenario: CI audit fails on known vulnerability
- Given the GitHub Actions checks job is running
- And Cargo.lock includes a dependency with a known RustSec advisory
- When `cargo audit` executes
- Then the audit step SHALL fail
- And the checks job SHALL fail
- And the workflow logs SHALL include the advisory details

## ADDED Requirements

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
