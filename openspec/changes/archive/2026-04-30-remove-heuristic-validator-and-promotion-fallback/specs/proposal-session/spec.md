## ADDED Requirements

### Requirement: Proposal workflow guidance must align with explicit structural validation

Proposal workflow guidance SHALL tell authors to provide explicit structure for validation-relevant fields rather than relying on wording that a validator might interpret heuristically.

When verification ownership, executable surfaces, or similar validation-relevant concerns matter, the guidance SHALL require an explicit marker or declared field recognized by the validator.

#### Scenario: proposal guidance requires explicit markers rather than wording cues
- **GIVEN** a proposal introduces validation-relevant concerns such as verification ownership or executable surfaces
- **WHEN** the workflow guidance instructs the author how to express them
- **THEN** the guidance asks for explicit markers or fields recognized by the validator
- **AND** it does not tell the author that descriptive wording alone is sufficient for machine validation
