## 1. Implementation
- [ ] 1.1 Update parallel completion event emission to allow MergeWait-only completion (verify: AllCompleted is sent when all queued changes finish and only MergeWait remains; inspect src/parallel/mod.rs for removal of MergeWait gating).
- [ ] 1.2 Adjust header status labels to Ready/Running and hide status in Stopped/Error modes (verify: header text in src/tui/render.rs matches new rules).
- [ ] 1.3 Simplify status line to progress bar + elapsed only, removing change/status text (verify: render_status output in src/tui/render.rs).
- [ ] 1.4 Update progress calculation to use selected (x) changes in all modes and accumulate running elapsed time (verify: progress and elapsed calculations in src/tui/render.rs).
- [ ] 1.5 Update or add tests covering MergeWait completion and new header/status display rules (verify: cargo test or relevant unit tests in src/tui and src/parallel).
