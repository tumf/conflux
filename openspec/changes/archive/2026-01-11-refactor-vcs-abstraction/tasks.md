## 1. Preparation

- [x] 1.1 `src/vcs/` directory structure created
- [x] 1.2 `VcsError` type defined in `src/vcs/mod.rs`

## 2. Common Command Helper Extraction

- [x] 2.1 `run_vcs_command()` implemented in `src/vcs/commands.rs`
- [x] 2.2 Output parsing utilities added (shared error handling patterns)

## 3. Jujutsu Implementation Migration

- [x] 3.1 `jj_commands.rs` → `src/vcs/jj/commands.rs` moved
- [x] 3.2 `jj_workspace.rs` → `src/vcs/jj/mod.rs` moved
- [x] 3.3 Refactored to use common helpers

## 4. Git Implementation Migration

- [x] 4.1 `git_commands.rs` → `src/vcs/git/commands.rs` moved
- [x] 4.2 `git_workspace.rs` → `src/vcs/git/mod.rs` moved
- [x] 4.3 Refactored to use common helpers

## 5. Trait and Public API Cleanup

- [x] 5.1 `vcs_backend.rs` content integrated into `src/vcs/mod.rs`
- [x] 5.2 `WorkspaceManager` trait methods return `VcsResult<T>`
- [x] 5.3 Old files removed (no re-export needed - direct vcs module usage)

## 6. Error Type Integration

- [x] 6.1 VCS error variants mapped via `From<VcsError> for OrchestratorError`
- [x] 6.2 Legacy error variants kept for backward compatibility

## 7. Testing and Validation

- [x] 7.1 All tests pass (`cargo test` - 245 unit + 26 e2e + 3 compatibility)
- [x] 7.2 No clippy warnings (`cargo clippy`)
- [x] 7.3 E2E tests verified
