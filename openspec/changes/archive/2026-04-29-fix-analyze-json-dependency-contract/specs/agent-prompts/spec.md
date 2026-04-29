## MODIFIED Requirements

### Requirement: Dedicated analyze and resolve skills MUST own fixed operation guidance

The dedicated `cflx-analyze` skill MUST define the allowed dependency target set for analyze output as the current queued change IDs plus any explicitly supplied in-flight change IDs. Rust prompt builders MAY provide those IDs as runtime context, but analyze guidance MUST NOT leave the allowed dependency universe ambiguous.

#### Scenario: Analyze guidance declares closed-world dependency targets
- **GIVEN** dependency analysis is executed through the standard orchestrator path
- **WHEN** the analyze prompt is assembled
- **THEN** the authoritative guidance from `cflx-analyze` states that `dependencies` may reference only queued change IDs and explicit in-flight change IDs
- **AND** it forbids returning unrelated active/repo-local change IDs as dependency targets
