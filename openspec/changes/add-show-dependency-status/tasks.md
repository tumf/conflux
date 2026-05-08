## Implementation Tasks

- [ ] Extend show data modeling in `src/openspec_cmd.rs` so `ShowInfo` can carry parsed dependency IDs and `DependencyStatusInfo` entries for non-archived change details. (verification: unit - `cargo test openspec_cmd --lib` includes a show fixture proving `OpenSpecManager::show_change()` returns dependency statuses for active dependent changes; completion condition: `ShowInfo` exposes the same status labels available to list output without reading logs or out-of-worktree durable state.)
- [ ] Render dependency statuses in human-readable `cflx openspec show <change-id>` output only when dependencies exist. (verification: unit - `cargo test openspec_cmd --lib` covers dependent and independent show output rendering in `src/openspec_cmd.rs`; completion condition: dependent output contains `Dependencies: feature-a [pending]` style text and independent output contains no empty `Dependencies:` line.)
- [ ] Add structured dependency status output to `cflx openspec show --json <change-id>`. (verification: unit - `cargo test openspec_cmd --lib` validates the JSON object includes parseable dependency status entries with IDs and labels; completion condition: consumers do not need to scrape `proposal` text to discover dependency status.)
- [ ] Preserve `--deltas-only` behavior for `cflx openspec show`. (verification: unit - `cargo test openspec_cmd --lib` covers `show_change(..., deltas_only = true)` or command output proving dependency fields are omitted from deltas-only results; completion condition: deltas-only output remains limited to spec deltas and existing metadata.)
- [ ] Verify all show dependency statuses use the same workspace-local classification rules as list output. (verification: unit - `cargo test openspec_cmd --lib` covers active, in-flight, archived, and missing dependency targets using fixtures under temporary `openspec/changes`, `.conflux-inflight`, and archive directories; completion condition: show labels match list labels for `pending`, `running`, `done`, and `missing`.)

## Future Work

- Add equivalent dependency status presentation to TUI or Web UI detail panels if users need it later.

## Final Validation

Expected archive gate: `cflx openspec validate add-show-dependency-status --archive-gate`
