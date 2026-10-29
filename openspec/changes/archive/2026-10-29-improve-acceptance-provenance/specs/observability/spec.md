## ADDED Requirements

### Requirement: Acceptance settlement log line carries provenance without values

Every Acceptance settlement MUST emit one structured log line carrying the same provenance fields the change row publishes (`decision_source`, `reviewer_started`, `reason_code`, `defect_code`, `verification_id`, `candidate_commit`, `proposal_content_id`, `program`, `argument_count`, `env_names`, `remediation`, divergence status). The line MUST NOT contain command argument values, environment values, reviewer prompt text, or captured process output, and it MUST be observability only: its absence or deletion MUST NOT change any workflow decision.

#### Scenario: Settlement line is bounded and redacted

- **GIVEN** a gate whose declaration carries a sentinel argument and a sentinel assignment value
- **WHEN** Acceptance settles on that gate
- **THEN** the settlement log line names the program, the argument count, and the environment names
- **AND** neither sentinel appears in the line

#### Scenario: Log loss changes nothing

- **GIVEN** the log files and the Acceptance state root are deleted after a settlement
- **WHEN** the owner computes the change's next action
- **THEN** the action is the one it would have computed with the files present
