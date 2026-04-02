## Implementation Tasks

- [x] Add persistent/in-memory server project sync-state fields and serialization support in registry and remote DTOs (verification: `src/server/registry.rs` and `src/remote/types.rs` expose sync-state metadata in project snapshots)
- [x] Add a server background polling loop that periodically refreshes per-project remote sync state without invoking `git/sync` (verification: server module contains a periodic monitor task and automated tests cover non-invasive checks)
- [x] Implement git-based ahead/behind state computation for managed server projects, including `up_to_date`, `ahead`, `behind`, `diverged`, and `unknown` outcomes (verification: server/API tests cover each state classification and error path)
- [x] Expose sync-state metadata via REST state snapshots and WebSocket `full_state` updates used by remote clients (verification: API/WebSocket tests assert the new fields are present)
- [x] Update server-mode dashboard and/or remote client rendering to display ahead/behind state clearly for each project (verification: relevant UI state/render tests assert display-ready sync information is consumed)
- [x] Add regression tests ensuring monitoring never triggers `git/sync` or `resolve_command` automatically (verification: tests fail if periodic checks mutate sync state through active reconciliation)
- [x] Run repository validation commands for the eventual implementation (`cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`) (verification: commands succeed before implementation merge)

## Future Work

- Add operator controls for manually refreshing sync state on demand
- Consider webhook-assisted refresh after the polling baseline is stable
- Consider auto-sync policies only after monitoring proves reliable and observable
