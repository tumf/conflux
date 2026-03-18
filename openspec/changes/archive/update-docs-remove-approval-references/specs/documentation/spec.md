## MODIFIED Requirements
### Requirement: README.md Content Accuracy

README.md SHALL accurately document all current features, commands, workflow states, and project structure. It MUST NOT describe removed approval-flow interactions such as `@`-based approval, approval-state tables, or approve/unapprove prerequisites as current behavior.

#### Scenario: Post-approval-removal README workflow

- **WHEN** a user reads the README.md
- **THEN** they understand current change selection and execution flow without any approval prerequisite
- **AND** no active instruction tells them to press `@` to approve a change
- **AND** no table or narrative presents approval as a current state dimension

### Requirement: Japanese Localization

The project SHALL provide README.ja.md as a complete Japanese translation of the current README workflow and examples.

#### Scenario: Approval-free workflow parity

- **WHEN** README.ja.md is compared with README.md
- **THEN** both describe the same current non-approval workflow
- **AND** neither document presents `@` approval, approve/unapprove APIs, or approval prerequisites as active behavior

## ADDED Requirements
### Requirement: Usage and Development Guide Workflow Accuracy

`docs/guides/USAGE.md` and `docs/guides/DEVELOPMENT.md` SHALL describe only the current workflow, commands, hooks, and keybindings. They MUST NOT present removed approval-flow concepts as active user behavior.

#### Scenario: Usage guide excludes removed approval interactions

- **WHEN** a user follows `docs/guides/USAGE.md`
- **THEN** they are not instructed to approve changes with `@`
- **AND** examples describe current selection and execution behavior only

#### Scenario: Development guide excludes removed approval hooks and APIs

- **WHEN** a developer reads `docs/guides/DEVELOPMENT.md`
- **THEN** active hooks, API examples, and architectural descriptions match the current product
- **AND** removed approval interactions are either absent or clearly marked as historical context rather than current behavior
