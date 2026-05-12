## Implementation Tasks

- [ ] Add the runtime module structure for three-level state under `src/runtime/` or an equivalent crate module path, and expose it from the crate root only as needed by tests and future integrations. (verification: unit - `cargo test runtime::` discovers and runs tests in `src/runtime/mod.rs` or equivalent module files)

- [ ] Define strongly typed identifiers and snapshots for Orchestrator, Project, and Proposal runtime layers, including a compatibility bridge from existing OpenSpec change IDs to proposal IDs. (verification: unit - `cargo test runtime::ids runtime::snapshot` asserts stable string round-tripping and project/proposal nesting in `src/runtime/ids.rs` and `src/runtime/snapshot.rs`)

- [ ] Define `ProposalStatus` as a single enum covering not queued, queued, dependency blocked, applying, accepting, rejecting, stalled, archiving, merge wait, resolving, merged, rejected, failed, and stopped states, with payloads for workspace refs, blockers, attempts, and revisions where required. (verification: unit - `cargo test runtime::proposal` covers `src/runtime/proposal.rs` construction and status-label tests without separate queue/activity/wait/terminal fields)

- [ ] Define `ProjectRuntimeState` with project status, proposal map, project-local queue/dispatch view derivation, and a single project-level base-lane owner for merge/resolve/rejecting work. (verification: unit - `cargo test runtime::project` covers `src/runtime/project.rs` reducer/view tests that reject or defer simultaneous base-lane ownership within one project)

- [ ] Define `OrchestratorRuntimeState` with global lifecycle status and project aggregation while keeping proposal lifecycle details inside projects. (verification: unit - `cargo test runtime::orchestrator` covers `src/runtime/orchestrator.rs` snapshot tests deriving global running/stopped/error status from project-level events)

- [ ] Add scoped runtime events for orchestrator, project, and proposal transitions and implement a pure reducer that mutates only the in-memory runtime model. (verification: unit - `cargo test runtime::reducer` covers `src/runtime/reducer.rs` transitions for queue -> applying -> accepting -> archiving -> merge wait -> resolving -> merged)

- [ ] Implement stale-event and terminal-state idempotency rules for proposal lifecycle transitions. (verification: unit - `cargo test runtime::reducer::terminal` or equivalent tests in `src/runtime/reducer.rs` show stale apply/archive/resolve/merge events cannot regress merged or rejected proposals)

- [ ] Implement derived compatibility views for queued proposals, stalled proposals, merge-wait proposals, resolve-wait proposals, rejected proposals, merged proposals, and project dispatch candidates without making those views canonical storage. (verification: unit - `cargo test runtime::snapshot runtime::project::dispatch_view` asserts derived views update from `ProposalStatus` and no separate canonical sets are required)

- [ ] Document constitution compliance in code-level design notes or module docs: runtime state is in-memory orchestration/observability state and MUST NOT become durable workflow-control input for resume, acceptance, archive, or next-action routing. (verification: manual - reviewer checks `src/runtime/mod.rs` module docs and confirms `src/server/db.rs` and out-of-worktree files are not used to persist runtime lifecycle control)

- [ ] Keep existing execution paths behaviorally unchanged while introducing the model. (verification: integration - run `cargo test orchestration::state parallel:: server:: runtime::` or the repository-supported equivalent command after implementation)

## Future Work

- Rewire the parallel project scheduler to consume `ProjectRuntimeState` dispatch views.
- Migrate TUI/Web/API snapshots to read from the new runtime snapshot as read-only consumers.
- Separate server project configuration persistence from runtime project state.
- Remove obsolete serial runtime path and replace serial execution with project scheduler concurrency `1` behavior.
- Retire or facade the legacy `OrchestratorState` after all consumers migrate.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate introduce-three-level-runtime-state --archive-gate`
