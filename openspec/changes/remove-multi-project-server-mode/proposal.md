---
change_type: implementation
priority: high
dependencies: []
references:
  - src/server/
  - src/service/
  - src/remote/
  - src/web/
  - dashboard/
  - openspec/specs/cli/spec.md
  - openspec/specs/web-monitoring/spec.md
  - openspec/specs/remote-control-api/spec.md
verifications:
  - id: repository-checks
    requirement: The obsolete multi-project server surfaces are absent while local web monitoring and API v2 remain functional
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test, clippy, feature-matrix checks, and CLI/API regression test output
    rerun: make lint && make test && cargo check --no-default-features
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Remove obsolete multi-project server mode

**Change Type**: implementation

## Problem / Context

Conflux still ships an obsolete multi-project server product comprising `cflx server`, `cflx service`, `cflx project`, remote-client TUI options, a persistent server backend, and a separate React dashboard. This surface increases binary, dependency, build, release, documentation, and maintenance cost despite no longer being used.

The active local web-monitoring product is separate: local TUI and `cflx run` use `--web`, `src/web/**`, and the versioned `/api/v2` remote-control API. Removing the obsolete product must not remove or weaken those capabilities.

## Proposed Solution

Remove the obsolete multi-project server product end-to-end:

- remove the `server`, `service`, and `project` commands and remote TUI `--server*` options;
- make TUI local-only and remove server-client-only code from `src/remote/**`;
- remove `src/server/**`, `src/service/**`, `dashboard/**`, server persistence, server proposal sessions, server terminals, project registry, and server worktree/git-sync APIs;
- remove server-only configuration, defaults, dependencies, build hooks, package contents, CI/release steps, OpenAPI surfaces, tests, and documentation;
- retain local `--web` options, `src/web/**`, `web-monitoring`, `/api/v2`, its authentication/CORS guarantees, and the OpenAPI generator;
- prune dependencies only after confirming they have no remaining local web-monitoring or core use.

This is one atomic change because partially removing the CLI, backend, dashboard, or canonical contracts would leave unsupported entrypoints or build-time coupling. The retained local web path is protected by explicit regression verification.

## Acceptance Criteria

- `cflx --help` no longer advertises `server`, `service`, or `project`.
- Top-level and `cflx tui` help no longer advertise `--server`, `--server-token`, or `--server-token-env`; invocations using removed surfaces fail as Clap usage errors before logging, locking, network access, or orchestration side effects.
- `src/server/**`, `src/service/**`, and `dashboard/**` are absent, and no production module references the removed server daemon, project registry, persistent server DB, proposal-session server, terminal server, or dashboard assets.
- Server-only configuration and defaults are no longer parsed or exposed. Unrelated configuration remains compatible.
- Normal build, package, CI, and release paths do not invoke Node/npm or require dashboard artifacts.
- `cflx run --web` and local `cflx tui --web` retain their existing flags and startup behavior.
- `/api/v2` state, observation streams, command handling, authentication, binding, and exact-origin CORS retain their canonical contracts and regression coverage.
- Default-feature and `--no-default-features` Rust builds compile; default tests and clippy pass.
- Current documentation and canonical specs describe only retained behavior; archived historical changes remain unchanged.

## Explicit Completion Conditions

- Repository search finds no live CLI/config/build/documentation references that claim the removed server product is available, except intentional migration/rejection tests or archived history.
- CLI integration tests execute the removed commands/options and prove non-zero side-effect-free rejection, while retained `--web` help and API tests still execute real code paths.
- `cargo check --no-default-features`, `cargo clippy -- -D warnings`, and `cargo test` succeed.
- Packaging metadata and build scripts no longer include or inspect `dashboard/dist`.
- Strict OpenSpec and archive-gate validation succeed for this change.

## Migration and Compatibility

This is an intentional breaking removal. No compatibility shim, deprecated alias, daemon data migration, or automatic service uninstall is added. Existing installed OS services must be stopped/uninstalled by operators before upgrading; documentation provides concise manual cleanup guidance. Existing server data is left untouched to avoid destructive upgrade behavior.

## Out of Scope

- Removing or redesigning local `--web` monitoring.
- Removing `src/web/**`, `/api/v2`, `web-monitoring`, or `openapi-gen`.
- Changing local orchestration, worktree behavior, TUI interaction, API v2 semantics, authentication, or CORS.
- Deleting archived OpenSpec changes or user-owned server data/service definitions automatically.
