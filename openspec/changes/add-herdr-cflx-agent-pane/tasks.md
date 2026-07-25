## Implementation Tasks

- [ ] Add the minimal Herdr plugin manifest and POSIX launcher under `plugins/herdr/`; declare plugin id `tumf.cflx`, pane entrypoint `tui`, macOS/Linux support, and invoke `cflx tui` without changing cwd. (verification: unit - `cargo test herdr_plugin_manifest` parses `plugins/herdr/herdr-plugin.toml` and asserts the exact id, pane id, supported platforms, and launcher argv)
- [ ] Wire launcher lifecycle reporting so the current `HERDR_PANE_ID` is reported with source `plugin:tumf.cflx`, agent label `cflx`, and state `working`, then always released on normal exit, signal-driven exit, or launch failure while preserving the `cflx tui` exit status. (verification: integration - `cargo test herdr_plugin_launcher` runs `plugins/herdr/run-tui.sh` against fake Herdr and cflx executables and asserts report/release calls, cwd, arguments, and exit codes)
- [ ] Reject missing Herdr pane context, missing Herdr CLI context, and unavailable `cflx` before leaving stale lifecycle authority. (verification: integration - `cargo test herdr_plugin_launcher_errors` asserts non-zero exits, concise stderr, and no unmatched report from `plugins/herdr/run-tui.sh`)
- [ ] Document local `herdr plugin link`, pane opening, prerequisites, supported platforms, and uninstall/unlink steps in `docs/guides/USAGE.md`. (verification: integration - `cargo test herdr_plugin_documentation` asserts `docs/guides/USAGE.md` contains the link, open, prerequisite, and unlink commands; manual confirmation with `herdr agent list` is intentional for the terminal UI label)
- [ ] Run repository quality gates and keep default tests below the project's one-second unit-test threshold; mark only genuinely unavoidable slow coverage as heavy. (verification: integration - `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test`)

## Future Work

- Publish the plugin from a public GitHub repository and add the `herdr-plugin` topic after separate maintainer approval.
- Add richer phase/status synchronization only if Herdr visibility beyond the single `working` lifecycle is requested.
- Add Windows support when Conflux provides a supported Windows release and launcher contract.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate add-herdr-cflx-agent-pane --archive-gate`
