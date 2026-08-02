# Design: Isolate and remove multi-project server mode

## Boundary

Two HTTP-capable products currently coexist:

1. The obsolete multi-project product: `cflx server`, service/project commands, remote TUI, `src/server/**`, persistent registry/database, and `dashboard/**`.
2. The retained single-instance local web product: `--web`, `src/web/**`, and `/api/v2` remote control attached to a local TUI or `cflx run` process.

The deletion follows ownership rather than dependency names. Shared crates or types remain whenever product 2 still consumes them.

## Removal Sequence

1. Add failing/retained CLI and API regression assertions.
2. Remove public CLI entrypoints and remote-TUI branching.
3. Remove server/service modules and server-client code.
4. Remove dashboard/build/package integration.
5. Remove server configuration and now-unused dependencies.
6. Synchronize canonical specs and current documentation.
7. Run both Cargo feature configurations and the complete local quality gates.

This order first closes entrypoints, then deletes unreachable implementation, reducing the chance of leaving a callable partial server.

## Safety Decisions

### Fail removed CLI surfaces at parsing

No hidden compatibility commands are retained. Clap returns a usage error before repository lock acquisition, logging initialization, network connection, or orchestration startup. Tests must assert both the non-zero result and absence of representative side effects.

### Preserve user-owned state

The binary does not uninstall OS services or delete server data during upgrade. Destructive cleanup remains an explicit operator action documented in migration guidance.

### Preserve local Web by positive verification

Absence checks alone cannot prove correct separation. Verification must positively parse/start retained `--web` paths and execute API v2 route/auth/stream/command tests. A placeholder router or compile-only remnant does not satisfy acceptance.

### Prune dependencies last

`axum`, `tower`, `tower-http`, `utoipa`, and related feature wiring may serve retained API v2. `reqwest`, WebSocket, SQLite, PTY, QR, and local-address crates are removed only after caller analysis and feature-matrix compilation establish they are server-only.

## Spec Promotion

The delta removes obsolete canonical capabilities and modifies the web-monitoring contract to define the retained dashboard/API boundary without the deleted standalone React dashboard. `remote-control-api` remains unchanged because it is explicitly retained.

Archived changes are historical evidence and are not rewritten.
