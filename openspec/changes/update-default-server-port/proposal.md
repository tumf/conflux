# Change: update default server port to 39876

## Problem/Context

- The current default `cflx server` listen port is `9876`.
- The user wants the port to remain configurable, but the default should move to a higher, less commonly expected fixed port.
- The repository already exposes server port overrides through global config `server.port` and CLI `cflx server --port`, so the requested change is limited to the default value and related user-facing defaults.

## Proposed Solution

- Change the default server port from `9876` to `39876`.
- Preserve existing override behavior for `server.port` and `cflx server --port <PORT>`.
- Update CLI and configuration specifications so that all default-port references use `39876`.
- Update related examples and default endpoint text for local remote-client usage so documentation stays consistent with the new default.

## Acceptance Criteria

- With no explicit `server.port` configured and no `--port` override, `cflx server` uses port `39876`.
- With explicit global config or CLI override, the configured port still takes precedence over the default.
- User-facing specification text and examples referencing the default local server endpoint use `127.0.0.1:39876`.

## Out of Scope

- Changing server bind defaults or authentication behavior.
- Switching the server to OS-assigned dynamic ports.
- Redesigning remote server discovery beyond updating the default fixed port.
