## ADDED Requirements

### Requirement: Acceptance findings MUST define an actionable structured repair contract

The JSON-primary Acceptance FAIL verdict MUST support structured repository findings with a stable non-empty `id`, `severity` of `major` or `minor`, non-empty `summary`, concrete non-empty `evidence`, one or more repository-relative `required_changes`, and one or more repository-relative `verification` expectations. Each required change and verification entry MUST identify a file and describe the expected behavior or proof. Both severities MUST block PASS.

The reviewer MUST reuse the same finding ID while reporting the same underlying defect, regardless of changed prose, evidence, line numbers, or additional observed locations. Canonical guidance MUST NOT use mutable summary or evidence text as the identity. Runtime MAY accept legacy string findings for backward compatibility, but a valid structured finding MUST NOT be reduced to its normalized comparison identity before Apply.

Malformed structured findings MUST NOT silently degrade to path-only repair instructions. They MUST follow bounded Acceptance protocol handling.

#### Scenario: detailed repository finding reaches repair contract

- **GIVEN** Acceptance identifies a repository-fixable secret-value verification gap
- **WHEN** it emits a structured FAIL finding
- **THEN** the finding identifies a stable ID, severity, summary, concrete evidence, required implementation files, and required verification files
- **AND** the complete fields remain available to runtime and Apply
- **AND** both `major` and `minor` prevent PASS

#### Scenario: same defect retains finding ID

- **GIVEN** a later review observes the same underlying defect at changed line numbers or with additional evidence
- **WHEN** Acceptance emits its next FAIL
- **THEN** it reuses the prior finding ID
- **AND** changed summary or evidence does not create a new repair opportunity

#### Scenario: malformed structured finding is not made lossy

- **GIVEN** an object finding omits evidence, required changes, verification, or a valid repository-relative path
- **WHEN** runtime parses the verdict
- **THEN** runtime does not convert it to a normalized path-only string
- **AND** it requests bounded protocol correction rather than dispatching ambiguous Apply work

#### Scenario: legacy string finding remains compatible

- **GIVEN** an existing Acceptance integration returns a FAIL with string findings
- **WHEN** runtime parses the verdict
- **THEN** it preserves each complete original string for Apply
- **AND** it derives a separate fallback identity for retry comparison
- **AND** legacy syntax remains accepted without weakening valid structured findings

### Requirement: Apply repair prompts MUST prioritize complete latest findings

An Apply invocation following Acceptance FAIL MUST receive the complete latest open finding payload exactly once in an explicitly untrusted machine-readable block. The prompt MUST prioritize those findings and runtime repair instructions above completed proposal tasks, prior implementation narrative, and other bounded context. It MUST NOT replay all prior Acceptance attempts or substitute normalized identities for actionable details.

Apply guidance MUST require a remediation evidence mapping for every required change and verification expectation. Additional changed files MUST have an explicit relationship to an open finding. Apply MAY claim remediation, but MUST NOT close a finding, claim Acceptance PASS, or treat a runtime-owned checkbox as semantic acceptance.

#### Scenario: normalized identity cannot replace repair detail

- **GIVEN** a finding contains concrete evidence and required verification for a repository path
- **AND** runtime derives a compact identity for retry comparison
- **WHEN** the next Apply prompt is built
- **THEN** the prompt contains the complete finding exactly once
- **AND** the compact identity does not replace evidence, required changes, or verification

#### Scenario: repair mode does not rediscover completed proposal work

- **GIVEN** proposal implementation tasks are complete and Acceptance reports one open finding
- **WHEN** Apply begins the repair invocation
- **THEN** the latest finding is its primary work scope
- **AND** completed proposal tasks remain constraints rather than new work candidates
- **AND** unrelated changes require an explicit finding relationship

#### Scenario: remediation claim remains pending Acceptance

- **GIVEN** Apply changes every declared file and records remediation evidence
- **WHEN** Apply completes
- **THEN** the finding is only remediation-claimed
- **AND** only a later canonical Acceptance result can close it or authorize PASS
