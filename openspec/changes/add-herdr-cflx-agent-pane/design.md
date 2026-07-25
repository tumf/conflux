# Design: Herdr cflx Agent Pane

## Decision

Ship an out-of-process Herdr plugin package in the Conflux repository. Do not modify Conflux's CLI dispatch or introduce a Conflux plugin registry.

Herdr already owns plugin manifests, managed PTY panes, invocation context, and agent lifecycle reporting. The plugin therefore needs only a manifest and a small launcher.

## Runtime Flow

1. Herdr opens the plugin's `tui` pane in the selected workspace cwd.
2. The launcher validates `HERDR_PANE_ID`, `HERDR_BIN_PATH`, and `cflx` availability.
3. The launcher calls `HERDR_BIN_PATH pane report-agent` for the current pane with source `plugin:tumf.cflx`, label `cflx`, and state `working`.
4. The launcher starts `cflx tui` without changing cwd and forwards terminal signals.
5. On every exit path after registration, the launcher calls `pane release-agent` and exits with the TUI's status.

The launcher cannot use a final `exec cflx tui` because cleanup must run after process exit. It should remain a minimal signal-forwarding parent and avoid interpreting Conflux output.

## State Boundary

Herdr lifecycle state is presentation-only. The plugin may report that the pane is occupied by `cflx`, but Conflux does not read Herdr state. Workspace files, git state, and base-branch comparison remain the only authoritative workflow inputs.

## Testing

Use temporary fake executables rather than a live Herdr server. A fake Herdr command records argv; a fake `cflx` records cwd and argv and returns controlled statuses. This proves observable launcher behavior without credentials, sockets, or long-running TUI interaction.

The tests must cover:

- manifest contract;
- exact label `cflx`;
- unchanged cwd and exact `tui` argument;
- report before launch and release after exit;
- non-zero status forwarding;
- missing-context and missing-command failures;
- cleanup after a forwarded termination signal.

## Compatibility

Set `min_herdr_version` to the oldest verified release supporting plugin panes and `pane report-agent`/`pane release-agent`. Declare only macOS and Linux until Conflux has a Windows distribution contract.
