## Implementation Tasks

- [ ] Add parallel runtime mode, availability, maximum concurrency, VCS backend, and per-change eligibility/reason to capabilities and coherent state projection. (verification: unit - `cargo test --features web-monitoring parallel remote_control_api` verifies projection fixtures cover sequential/parallel, unavailable Git, eligible, uncommitted, active, and final rows; verification-id: remote-parallel-tests)

- [ ] Add a revision-fenced parallel-toggle command routed through the shared operator service with Select/Stopped guards and TUI-equivalent ineligible-mark cleanup. (verification: integration - `cargo test --features web-monitoring parallel remote_control_api` verifies TUI/v2 parity tests compare changed marks, mode, feedback, invalid-mode no-op/failure, and idempotent replay; verification-id: remote-parallel-tests)

- [ ] Add an atomic bulk execution-mark command that classifies one snapshot, chooses one target state from eligible rows, updates Running queue intent consistently, clears applicable NEW attention, and returns changed IDs plus stable exclusion reasons. (verification: integration - `cargo test --features web-monitoring parallel remote_control_api` verifies bulk tests cover all-mark/all-unmark, active/rejected/uncommitted exclusions, zero eligible, stale revision, and no partial effects; verification-id: remote-parallel-tests)

- [ ] Fence start against the full marked eligibility set so parallel start is all-or-nothing and returns actionable target-ineligible details. (verification: integration - `cargo test --features web-monitoring parallel remote_control_api` verifies start tests prove one ineligible target prevents scheduler spawn and leaves all marks/queue intents coherent; verification-id: remote-parallel-tests)

- [ ] Register commands, schemas, capabilities, events, and parity fixtures in the v2 API and generated OpenAPI surface. (verification: integration - `cargo test --features web-monitoring parallel remote_control_api` verifies remote-control route/schema tests execute success, no-op, rejection, and replay paths; verification-id: remote-parallel-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-remote-parallel-control --archive-gate`

The implementation must also pass `cargo test --features web-monitoring parallel remote_control_api`.

## Future Work

- Scheduler optimization and removal of obsolete serial mode remain separate changes.
