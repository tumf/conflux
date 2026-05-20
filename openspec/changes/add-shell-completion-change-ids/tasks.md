## Implementation Tasks

- [x] Add a public `completion <shell>` command and supported shell enum in `src/cli.rs` without changing existing subcommand parsing. (verification: unit - parser tests in `src/cli.rs` cover zsh, bash, fish, powershell, and unsupported shell rejection)
- [x] Add a hidden internal `__complete change-ids` command in `src/cli.rs` with active/archived scope flags and prefix filtering arguments. (verification: unit - parser tests in `src/cli.rs` cover default, `--active`, `--archived`, combined scopes, and `--prefix`)
- [x] Implement side-effect-free change ID candidate discovery that reads active changes and optional archived changes from `openspec/changes/`, normalizes dated archived entries, sorts and de-duplicates logical IDs, and treats missing workspace as empty success. (verification: unit - candidate discovery tests cover active entries, invalid directories, archived direct entries, dated archive normalization, prefix filtering, de-duplication, and missing `openspec/changes`)
- [x] Wire `src/main.rs` so public completion generation and hidden candidate lookup run before logging/config/orchestration/TUI/server paths. (verification: integration - `tests/completion_command_tests.rs` runs completion and candidate commands with temporary `XDG_STATE_HOME` and asserts no `cflx/logs` directory is created)
- [x] Generate shell completion scripts for zsh, bash, fish, and powershell, including dynamic hooks that call `cflx __complete change-ids` for the required change-id surfaces. (verification: integration - generated script tests assert success, non-empty stdout, shell-specific markers, and internal candidate command references for each supported shell)
- [x] Implement command-specific candidate scope behavior for `run --change`, `openspec show`, `openspec validate`, and `openspec archive`, including comma-token behavior for `run --change` where feasible in each shell. (verification: integration/manual - candidate command tests prove active vs archived scope; generated script content tests prove the relevant surfaces are wired; manual shell checks validate comma-token behavior in at least bash and zsh)
- [x] Add binary-level integration tests for the hidden candidate command using temporary workspaces. (verification: integration - `tests/completion_command_tests.rs` covers active-only default, prefix filtering, archived inclusion on request, archived omission by default, dated archive normalization, and missing-workspace empty success)
- [x] Run formatting and targeted test gates. (verification: command - `cargo fmt --check` and `cargo test completion` pass)

## Future Work

- Add installation helper commands for shell startup files only if explicitly requested later.
- Add dynamic completion for non-change-id values such as spec IDs, project IDs, or git branches only after separate scoping.

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate. Expected archive gate: `cflx openspec validate add-shell-completion-change-ids --archive-gate`.
