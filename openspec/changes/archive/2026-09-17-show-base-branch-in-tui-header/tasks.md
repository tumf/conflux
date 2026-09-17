## Implementation Tasks

- [x] Capture the attached base branch once from the startup repository root, store it as optional presentation-only `AppState` state, and leave it unset without failing startup when Git reports detached or unavailable branch identity. (verification: unit - `cargo test header_base_branch --lib`; verification-id: tui-header-base-branch-tests)
- [x] Render a non-empty captured base branch as `[<branch>]` immediately after the project path and before `[dirty]`, reserving its terminal display width before middle-eliding the path while preserving existing header styling and right-aligned version behavior. (verification: unit - `cargo test header_base_branch --lib`; verification-id: tui-header-base-branch-tests)
- [x] Add focused state and final-buffer regression tests for ordering, capture immutability, missing-branch fallback, long-path elision, dirty/version coexistence, Unicode display widths, and tiny terminal widths. (verification: unit - `cargo test header_base_branch --lib`; verification-id: tui-header-base-branch-tests)

## Notes

- Capture: `AppState::base_branch` is a private `Option<String>` written only by `set_base_branch`, which trims and drops a blank result so a whitespace-only lookup can never render as `[]`. `src/tui/runner.rs` calls it once with `vcs::git::commands::get_current_branch(&repo_root)`, whose detached-HEAD answer is already `Ok(None)`; a `VcsError` is logged at debug and stored as `None`, so startup never fails on branch lookup.
- Rendering: the branch segment is reserved in the same display-column accounting as `Conflux`, the status label, and `[dirty]` before the project path's middle-elision budget is computed, so an over-wide path yields columns to the branch rather than the other way round. Nothing in `render_header` reads Git or the process current directory.
- Two of the added render tests failed on their first run and were fixed as test defects, not production ones. `header_base_branch_coexists_with_status_dirty_and_version_content` asserted the literal `"/projects/conflux [main]"` at width 80, where the longer `[Running 1]` label legitimately elides the path; it now asserts segment order and validates the rendered path through `assert_rendered_path_represents`. `header_base_branch_rendering_stays_bounded_at_narrow_widths` measured the row with `header_line`, which counts the blank placeholder cell a double-width grapheme leaves behind and so reported a 60-column row as 63; it now uses `header_text`, which advances over the placeholder.
- The proposal gained a `## Retired Scenarios` declaration for the four canonical `TUI Header Project Identity` scenarios this delta retitles. Without it `openspec_cmd::promotion::tests::every_pending_change_promotes_without_dropping_a_scenario` failed; with it the promotion suite passes. Every retitled scenario carries its predecessor's assertions forward, so no coverage is dropped.
- evidence: `cargo test header_base_branch --lib` — 8 passed, 0 failed.
- evidence: `cargo test` (default suite) — 4456 lib tests plus every integration target passed, exit status 0.
- evidence: `cargo fmt` applied; `cargo clippy --all-targets -- -D warnings` clean.
- evidence: `cflx openspec validate show-base-branch-in-tui-header --strict` passed.

## Final Validation

Archive validation is the authoritative final OpenSpec validation gate. Expected archive gate: `cflx openspec validate show-base-branch-in-tui-header --archive-gate`.
