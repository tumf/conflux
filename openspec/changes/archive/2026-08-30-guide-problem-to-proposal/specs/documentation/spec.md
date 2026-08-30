## ADDED Requirements

### Requirement: Bundled proposal guidance transforms problems into change contracts

The bundled `cflx-proposal` skill SHALL treat a reported problem as investigation input rather than a ready proposal and SHALL guide an agent to convert repository-backed findings into a permanent, implementation-ready change contract before drafting detailed proposal artifacts.

<!-- Expected canonical result after archive: documentation requirements will require `cflx-proposal` to teach problem investigation and change-contract formation, not only proposal formatting. -->

#### Scenario: Uncertain problem remains outside the proposal

- **GIVEN** the current behavior, root cause, approach, or acceptance criteria remain uncertain
- **WHEN** an agent follows the bundled proposal guidance
- **THEN** it gathers read-only repository evidence within the skill's existing proposal-only scope before drafting the proposal
- **AND** it does not create an `investigate and fix` implementation task
- **AND** it creates no proposal when the investigation establishes that no permanent change is required

#### Scenario: Verified findings become a permanent transition

- **GIVEN** repository evidence establishes the current behavior and root cause
- **WHEN** the agent drafts a proposal
- **THEN** it separates temporary diagnostics and local repairs from the permanent change
- **AND** it defines the observable final state, change boundary, preserved contracts, failure behavior, and repository-local acceptance
- **AND** the implementation tasks leave no new design decisions to the implementation agent

#### Scenario: Scope-relevant alternatives remain explicit

- **GIVEN** investigation considered multiple implementation approaches
- **WHEN** a rejected alternative changes the proposal scope or preserved contracts
- **THEN** the proposal records that rejected alternative and its consequence
- **AND** incidental exploration detail need not become proposal content

#### Scenario: Policy is encountered before proposal formatting

- **WHEN** an agent reads the bundled `cflx-proposal` skill from the beginning
- **THEN** the problem-to-proposal policy appears directly after `## Scope Restrictions (Proposal-Only)`
- **AND** it appears before `## Guardrails (Match Command Behavior)` and detailed proposal-format guidance
