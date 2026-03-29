# Change: Fix git/sync up-to-date check using post-pull SHA

**Change Type**: implementation

## Why

The `git_sync` endpoint compares `local_sha` with `remote_sha` **after** the pull phase has already fast-forwarded the local branch to match the remote. This means the two SHAs always match when the remote has new commits, causing `resolve_command` to be skipped unconditionally. Users pressing Sync in the Web UI see "already up-to-date" even when new remote changes exist that require reconciliation.

## What Changes

- Capture `pre_pull_sha` (the local branch SHA **before** the fetch/fast-forward) in the pull phase of `git_sync`
- Use `pre_pull_sha` instead of `local_sha_for_push` in the up-to-date comparison at line 1481 of `src/server/api.rs`
- When the bare repo is newly cloned, treat `pre_pull_sha` as empty so resolve always runs on first sync
- Add a regression test that pushes a remote commit after initial clone and verifies resolve runs on the next sync

## Impact

- Affected specs: `git-sync`
- Affected code: `src/server/api.rs` (`git_sync` function)

## Acceptance Criteria

- When the remote branch has new commits not yet seen locally, `resolve_command` MUST be invoked during sync
- When local and remote are already identical (no new remote commits), `resolve_command` MUST still be skipped
- Existing tests `test_git_sync_skips_resolve_when_already_up_to_date` and `test_git_sync_runs_resolve_when_shas_differ` continue to pass
- A new test verifies the "remote ahead" scenario triggers resolve
