## MODIFIED Requirements

### Requirement: Resolve Context Format

When resolve verification continues because merge completion is still incomplete, the continuation reason SHALL distinguish between true missing merge evidence and successful fast-forward integration.

#### Scenario: Fast-forward merge does not emit missing-merge-commits context

- **GIVEN** the resolve command exits successfully
- **AND** the change has been integrated into the base branch via fast-forward
- **WHEN** the system evaluates whether another resolve attempt is needed
- **THEN** `<resolve_context>` does not include `Missing merge commits for change_ids`
- **AND** the change is not scheduled for another resolve attempt based on merge-commit absence alone
