## ADDED Requirements

### Requirement: Development Documentation Covers Validation Workflow

Development documentation SHALL describe dependency vulnerability auditing as part of the standard validation workflow and clarify that commit-time hooks do not run audit automatically.

#### Scenario: Developer reviews audit workflow
- Given a developer opens `docs/guides/DEVELOPMENT.md`
- When they review the validation commands
- Then the document SHALL describe how to run `make audit` and `cargo audit`
- And it SHALL state that `make check` includes dependency auditing
- And it SHALL state that pre-commit or prek hooks do not run `cargo audit` automatically
