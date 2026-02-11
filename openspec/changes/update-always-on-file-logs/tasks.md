## 1. Implementation
- [ ] 1.1 Remove `tui --logs` option and related CLI help/tests; verify by checking `src/cli.rs` has no `logs` field in `TuiArgs` and tests for `--logs` are removed.
- [ ] 1.2 Add log file path resolution using `XDG_STATE_HOME` (fallback `~/.local/state`) with `project_slug` + `YYYY-MM-DD.log`; verify by unit tests that the path matches expected inputs.
- [ ] 1.3 Initialize file logging for both TUI and CLI run modes using the unified path; verify by code inspection in `src/main.rs` and by a test that creates a log entry and confirms file creation.
- [ ] 1.4 Implement daily rotation with 7-day retention per project directory; verify by unit tests that old dated files are deleted and current date file remains.

## 2. Validation
- [ ] 2.1 Run `cargo test` and confirm all tests pass.
