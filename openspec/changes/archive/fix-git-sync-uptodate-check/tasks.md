## Implementation Tasks

- [x] 1. Capture `pre_pull_sha` before fetch in `git_sync` (`src/server/api.rs`): when bare repo already exists, run `git rev-parse refs/heads/{branch}` before the fetch commands and store the result; when bare repo is newly cloned, set `pre_pull_sha = ""` (verification: `cargo build` succeeds)
- [x] 2. Replace the up-to-date comparison (`local_sha_for_push == remote_sha_for_push`) with `pre_pull_sha == remote_sha_for_push` at the check on line ~1481 (verification: `cargo test test_git_sync_skips_resolve_when_already_up_to_date` still passes)
- [x] 3. Add test `test_git_sync_runs_resolve_when_remote_ahead`: push a new commit to the remote after the initial bare clone, then call `git/sync` and assert `resolve_command_ran == true` (verification: `cargo test test_git_sync_runs_resolve_when_remote_ahead`)
- [x] 4. Run full test suite and clippy (verification: `cargo test && cargo clippy -- -D warnings`)
