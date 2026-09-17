## Implementation Tasks

- [ ] Capture the attached base branch once from the startup repository root, store it as optional presentation-only `AppState` state, and leave it unset without failing startup when Git reports detached or unavailable branch identity. (verification: unit - `cargo test header_base_branch --lib`; verification-id: tui-header-base-branch-tests)
- [ ] Render a non-empty captured base branch as `[<branch>]` immediately after the project path and before `[dirty]`, reserving its terminal display width before middle-eliding the path while preserving existing header styling and right-aligned version behavior. (verification: unit - `cargo test header_base_branch --lib`; verification-id: tui-header-base-branch-tests)
- [ ] Add focused state and final-buffer regression tests for ordering, capture immutability, missing-branch fallback, long-path elision, dirty/version coexistence, Unicode display widths, and tiny terminal widths. (verification: unit - `cargo test header_base_branch --lib`; verification-id: tui-header-base-branch-tests)

## Final Validation

Archive validation is the authoritative final OpenSpec validation gate. Expected archive gate: `cflx openspec validate show-base-branch-in-tui-header --archive-gate`.
