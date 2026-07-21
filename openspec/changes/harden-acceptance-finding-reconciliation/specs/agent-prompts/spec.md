## MODIFIED Requirements

### Requirement: cflx-accept MUST define a portable acceptance skill interface

The dedicated `cflx-accept` skill MUST provide operation identity, scoped acceptance guidance, and the portable acceptance verdict output interface without requiring a specific agent runtime or command template. Runtime-specific entrypoints MAY mirror this interface as adapters, but they MUST NOT be the authoritative source that an acceptance skill must inspect to produce a correct verdict.

The primary acceptance verdict contract MUST be a strict JSON object emitted as the final machine-readable verdict payload. The runtime MAY continue to accept legacy plain-text standalone lines such as `ACCEPTANCE: PASS` as a backward-compatible fallback, but canonical guidance MUST prefer the JSON contract.

Repository-fixable findings MUST be atomic and SHOULD include a stable finding code. Implementation defects and missing verification evidence MUST be separate findings when they can be resolved or verified independently. Acceptance MUST evaluate the current worktree before re-reporting a prior finding and MUST NOT add a broad duplicate finding that merely aggregates test work already owned by specific findings.

Apply guidance MUST treat runtime-owned acceptance finding text as immutable task identity metadata. The general rule allowing task descriptions to be refined MUST NOT apply to runtime-owned acceptance findings; remediation and verification evidence MUST be recorded separately.

#### Scenario: cflx-accept defines JSON-primary verdict contract

- **GIVEN** the acceptance prompt loads `cflx-accept`
- **WHEN** the skill describes the final verdict format
- **THEN** it defines a strict JSON verdict object as the primary machine-readable contract
- **AND** it documents plain-text standalone verdict markers only as backward-compatible fallback guidance
- **AND** it does not require runtime-specific command files or a particular command invocation mechanism to describe the verdict interface

#### Scenario: repo-local acceptance skills follow the same contract

- **GIVEN** repo-local acceptance-related skills under `skills/` are reviewed
- **WHEN** they describe acceptance output expectations
- **THEN** they reference the same JSON-primary verdict contract
- **AND** they do not redefine a conflicting text-only canonical output rule
- **AND** they do not treat runtime-specific command templates as the authoritative source for the skill interface

#### Scenario: acceptance emits atomic current-state findings

- **GIVEN** a prior FAIL reported implementation and verification defects
- **WHEN** acceptance reviews a newer worktree state
- **THEN** it re-evaluates each defect against current repository evidence
- **AND** it reports implementation and verification defects separately when independently actionable
- **AND** it does not add an aggregate finding that duplicates their verification work
- **AND** each repository finding includes a stable code when the reviewer can provide one

#### Scenario: apply preserves runtime-owned finding text

- **GIVEN** runtime persisted an acceptance finding as a follow-up task
- **WHEN** apply fixes and verifies that finding
- **THEN** apply marks the existing task complete without rewriting its finding text
- **AND** remediation or verification evidence is recorded separately
