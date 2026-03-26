## Implementation Tasks

- [x] Update the resumed `WorkspaceState::Archived` path in `src/parallel/dispatch.rs` so it hands off archive-complete semantics to merge handling instead of returning a silent success with no merge handoff (verification: targeted test around resumed archived dispatch result).
- [x] Keep parallel completion handling consistent for resumed archived workspaces so downstream state/event consumers treat them like freshly archived changes in CLI and TUI flows (verification: reducer or bridge test proving parallel resume reaches `MergeWait` rather than `NotQueued`).
- [x] Add regression coverage for restart after interrupted parallel archiving with mixed `Archiving` and `Archived` workspaces using the existing resume/state-detection test surface under `src/parallel/` and `src/tui/` (verification: targeted `cargo test` covering the mixed-state restart scenario).

## Future Work

- Validate the resumed archive/merge behavior in an end-to-end TUI restart harness if a stable interactive test fixture is added later.
