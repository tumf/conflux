## Implementation Tasks

- [ ] Parse dependency metadata into list output change records by extending `src/openspec_cmd.rs::ChangeInfo` and `OpenSpecManager::get_change_info()` to include dependencies from `crate::openspec::parse_proposal_metadata_from_file()` without changing `--specs` records. (verification: unit - `cargo test openspec_cmd --lib` includes a fixture proving frontmatter dependencies appear in list change records; completion condition: dependencies from proposal frontmatter are available to `cmd_list(false)` and specs list data structures remain unchanged)
- [ ] Add dependency status classification for list output using workspace-local active change ids, `.conflux-inflight`, and archived change ids, reusing or aligning with `src/dependency_targets.rs` semantics. (verification: unit - `cargo test openspec_cmd --lib` covers pending/running/done/missing classification; completion condition: list output can distinguish active, in-flight, archived, and missing dependencies without reading logs or external state)
- [ ] Render dependency status labels in `cflx openspec list` human-readable output as `<id> [done|running|pending|missing]`, omitting the line for changes with no dependencies. (verification: integration - `cargo test openspec_cmd --lib` captures `render_changes_output` or equivalent formatting for dependent and non-dependent fixtures; completion condition: output contains a dependency line only when dependencies exist and each dependency has exactly one status label)
- [ ] Preserve canonical specs output behavior for `cflx openspec list --specs`. (verification: unit - `cargo test openspec_cmd --lib` keeps or adds a specs-output assertion that no `Dependencies:` line appears in specs output; completion condition: `render_specs_output` output remains limited to spec name, path, and requirement count)
- [ ] Add regression coverage for body `## Dependencies` fallback so dependency status rendering honors existing proposal metadata compatibility. (verification: unit - `cargo test openspec_cmd --lib` includes a proposal without frontmatter dependencies and with body dependencies; completion condition: fallback dependency appears with the expected status)
- [ ] Run Rust formatting and targeted tests for the touched code paths. (verification: unit - `cargo fmt --check` and `cargo test openspec_cmd --lib`; completion condition: commands complete successfully or any failures are fixed or documented with repository-verifiable cause)

## Future Work

- Consider a separate proposal for machine-readable `cflx openspec list --json` output if downstream automation needs dependency status data.
- Consider a separate proposal for showing dependency status in the TUI change list.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-openspec-list-dependency-status --archive-gate`
