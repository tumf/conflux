## MODIFIED Requirements

### Requirement: Dedicated analyze and resolve skills MUST own fixed operation guidance

The dedicated `cflx-analyze` and `cflx-resolve` skills MUST become the primary source of fixed operation guidance for dependency analysis and conflict resolution respectively. Rust-side prompt builders MAY inject variable runtime context, but they MUST NOT remain the primary home of fixed analyze / resolve rules, output contracts, safety constraints, sequential merge protocol, or commit conventions.

#### Scenario: Analyze fixed guidance moves out of inline Rust prompt text

- **GIVEN** dependency analysis is executed through the standard orchestrator path
- **WHEN** the analyze prompt is assembled
- **THEN** fixed dependency-selection guidance comes from `cflx-analyze`
- **AND** Rust primarily contributes variable context such as candidate changes and progress
- **AND** the Rust-side prompt body does not restate the analyze selection rules or output contract as authoritative instructions

#### Scenario: Resolve fixed guidance moves out of inline Rust prompt text

- **GIVEN** conflict resolution or merge-finalization recovery is executed through the standard orchestrator path
- **WHEN** the resolve prompt is assembled
- **THEN** fixed conflict-resolution guidance comes from `cflx-resolve`
- **AND** Rust primarily contributes variable context such as conflict files, VCS state, merge plan, and retry history
- **AND** the Rust-side prompt body does not restate the resolve safety rules, sequential merge protocol, or commit conventions as authoritative instructions
