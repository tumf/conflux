## ADDED Requirements

### Requirement: Proposal planning assigns verification ownership

The proposal creation workflow MUST require behavior-changing proposals to plan how each requirement will be verified so projects using Conflux can manage verification coverage explicitly instead of assuming every requirement becomes a unit test.

#### Scenario: Proposal records non-unit verification intentionally

**Given**: A user requests a feature whose UX quality cannot be judged well by unit tests
**When**: the `cflx-proposal` skill drafts the proposal
**Then**: the proposal planning guidance instructs the author to assign a verification path such as `manual` or `benchmark`
**And**: the requirement is treated as intentionally covered rather than missing unit tests

#### Scenario: Proposal records unit verification when logic is local

**Given**: A user requests a feature whose behavior is primarily local decision logic
**When**: the `cflx-proposal` skill drafts the proposal
**Then**: the proposal planning guidance instructs the author to assign `unit` verification where appropriate
**And**: related tasks identify the repository-verifiable test path

### Requirement: Proposal skill standardizes verification coverage vocabulary

The proposal creation workflow MUST provide a standard vocabulary for verification planning so Conflux-driven projects can discuss coverage consistently across proposals.

#### Scenario: Standard verification types appear in proposal guidance

**Given**: a human uses the `cflx-proposal` skill to draft a change proposal
**When**: the skill explains how to plan verification
**Then**: the guidance recognizes `unit`, `integration`, `e2e`, `manual`, `benchmark`, and `not-testable`
**And**: the guidance explains that these values represent verification ownership rather than only automated test types

### Requirement: Proposal tasks reflect verification coverage plan

The proposal workflow MUST guide authors to write tasks that make verification ownership traceable from implementation planning.

#### Scenario: Tasks connect implementation work to verification path

**Given**: a proposal contains implementation tasks
**When**: the `cflx-proposal` skill drafts `tasks.md`
**Then**: the guidance instructs the author to record the planned verification path alongside repository-verifiable evidence
**And**: reviewers can tell whether a task is intended for unit, integration, e2e, manual, or benchmark verification
