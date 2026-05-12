## Implementation Tasks

- [ ] Update Running mode layout calculation in `src/tui/render.rs` so logs-enabled layout uses a dynamic logs height when the current changes list needs fewer rows than the available flexible area. (verification: unit - add or update `src/tui/render.rs` render-buffer tests and run `cargo test tui::render` to prove few-change tall-terminal logs expand beyond the old fixed allocation)
- [ ] Preserve the current logs height behavior for many-change cases by ensuring the logs panel retains the existing 20-row allocation when changes require the remaining area. (verification: unit - add or update `src/tui/render.rs` render-buffer tests and run `cargo test tui::render` to prove many-change layout keeps the logs panel at the existing fixed-height boundary)
- [ ] Preserve logs-disabled Running mode behavior by leaving the header / changes / status constraints unchanged when `logs_panel_enabled` is false. (verification: unit - add or update `src/tui/render.rs` render-buffer tests and run `cargo test tui::render` to prove no `Logs` panel is rendered when `logs_panel_enabled` is false)
- [ ] Keep layout changes scoped to TUI rendering and avoid introducing workflow-control state or persisted layout state. (verification: manual - inspect `git diff -- src/tui/render.rs` and confirm no durable workflow state, reducer routing, queue behavior, or orchestration-control files changed)
- [ ] Run focused and formatting verification after implementation. (verification: unit - `cargo test tui::render`; verification: manual - `cargo fmt --check`; verification: manual - `cargo clippy --all-targets --all-features -- -D warnings` or document pre-existing unrelated failures)

## Future Work

- Optional user-configurable panel ratios or explicit minimum/maximum log heights are intentionally deferred.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate update-tui-log-flex-layout --archive-gate`
