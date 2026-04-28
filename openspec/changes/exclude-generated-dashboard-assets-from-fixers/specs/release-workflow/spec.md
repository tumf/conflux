## MODIFIED Requirements

### Requirement: Local Validation Command Includes Audit

The project SHALL provide a standard local command for dependency vulnerability auditing and include it in the comprehensive local validation target.

When that comprehensive validation path runs pre-commit / prek compatible hooks against committed dashboard publish artifacts under `dashboard/dist/assets/`, fix-up hooks MUST NOT rewrite those generated asset files as part of ordinary local validation.

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

#### Scenario: Dashboard generated assets are not fixer-mutated during hook validation
- Given `dashboard/dist/assets/index-abc123.js` and `dashboard/dist/assets/index-def456.css` are committed publish artifacts produced by the dashboard build
- And the developer runs `pre-commit run --all-files` or the documented equivalent hook runner from the repository root
- When fix-up hooks such as `end-of-file-fixer` are evaluated
- Then those committed dashboard generated assets are excluded from unsafe auto-rewrite behavior
- And the hook run does not stop solely because it rewrote those generated asset files
