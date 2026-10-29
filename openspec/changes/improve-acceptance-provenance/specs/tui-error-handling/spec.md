## ADDED Requirements

### Requirement: Details popup renders acceptance provenance

The details popup opened with `Enter` on an `error`, `stalled`, or externally `blocked` row SHALL render, after the existing `Change:` and `Error:` lines, the lines `Decision:`, `Reason:`, `Verification:`, `Candidate:`, `Command:`, `Next:`, and `Source proposal:` from the change's published acceptance provenance when one exists, showing identities (first twelve hex characters of commit and blob IDs), counts, and environment names only. The copy text SHALL keep its two existing lines first and append the same lines. Rows without provenance SHALL render the existing content unchanged. The popup is presentation state and SHALL NOT be used as workflow-control input.

#### Scenario: Native stop renders its provenance

- **GIVEN** a `blocked` row whose Acceptance settled on `tool_unavailable` before any reviewer started
- **WHEN** the operator presses `Enter`
- **THEN** the popup shows `Decision: native_verification (reviewer not started)`, `Reason: tool_unavailable`, the verification ID and declaration path, the candidate commit and proposal content identity, the program and argument count, and `Next: satisfy_environment`

#### Scenario: Reviewer verdict renders distinctly

- **GIVEN** an `error` row whose last Acceptance settlement was a reviewer `fail`
- **WHEN** the operator presses `Enter`
- **THEN** the popup shows `Decision: reviewer` and `Next: repair_candidate`
- **AND** no command argument value or environment value appears in the popup or its copy text

#### Scenario: Divergence line is explicit

- **GIVEN** provenance with `candidate_divergence.status` `diverged`, `identical`, and `unknown`
- **WHEN** the popup renders each
- **THEN** the `Source proposal:` line reads `diverged from <source_ref>@<12 hex>`, `identical`, and `unknown (<reason>)` respectively
