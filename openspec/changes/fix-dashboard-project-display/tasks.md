## Implementation Tasks

- [ ] Add a server-mode project snapshot type that includes explicit `repo`, `branch`, and status metadata for dashboard consumers (verification: Rust server state serialization compiles and is used by `src/server/api.rs`)
- [ ] Update project snapshot construction in the server to derive `repo` from `remote_url`, preserve `branch`, and carry status/error fields from the registry entry (verification: `src/server/api.rs` project snapshot builder covers normal and fallback repo extraction)
- [ ] Use the normalized project snapshot shape in both `GET /api/v1/projects/state` and WebSocket `full_state` payloads (verification: shared state-building path used by both endpoints in `src/server/api.rs`)
- [ ] Update dashboard TypeScript API types and state handling to match the server payload contract used in server mode (verification: `dashboard/src/api/types.ts`, `dashboard/src/api/wsClient.ts`, and `dashboard/src/store/useAppStore.ts` consume the same project shape without undefined repo/branch fields)
- [ ] Verify project card and selected-project header rendering against the normalized fields (verification: `dashboard/src/components/ProjectCard.tsx` and `dashboard/src/App.tsx` display repo and branch from the updated project model)
- [ ] Add or update regression coverage for project snapshot serialization and/or dashboard state parsing for server mode project display (verification: targeted Rust and/or dashboard tests exercise repo/branch display data)
- [ ] Run repository validation commands after implementation (verification: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, and dashboard lint/typecheck command if available)

## Future Work

- Consider consolidating all server/dashboard project response types behind a single explicitly versioned API contract if more dashboard fields diverge in the future
