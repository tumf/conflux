# Change: Skip unnecessary resolve_command in git sync

**Change Type**: implementation

## Problem/Context

`git/sync` currently always runs `resolve_command` before push, even when the local branch already matches the remote branch after the pull phase. In this repository, `resolve_command` is an expensive AI-driven reconciliation step, so running it when no remote updates need reconciliation adds unnecessary latency and cost.

The current implementation already computes both `local_sha_for_push` and `remote_sha_for_push` in `src/server/api.rs`, which makes it possible to detect the no-op case without additional remote queries.

## Proposed Solution

Update `git/sync` so it checks whether reconciliation is actually needed before invoking `resolve_command`.

- After the pull phase, compare `local_sha_for_push` and `remote_sha_for_push`
- If the SHAs match, skip `resolve_command`
- In the same up-to-date case, skip the push operation as well and return a successful sync response indicating no reconciliation or push was needed
- Continue to run `resolve_command` for cases where the SHAs differ or when the remote branch does not yet exist

This keeps the existing safety behavior for divergent states while avoiding unnecessary AI work when the repository is already synchronized.

## Acceptance Criteria

- `git/sync` does not invoke `resolve_command` when the pulled local branch SHA matches the current remote branch SHA
- `git/sync` returns success for the already-up-to-date case without attempting a push
- `git/sync` still invokes `resolve_command` before push when local and remote SHAs differ
- Existing error handling for missing `resolve_command` configuration and push rejection remains intact
- Regression tests cover both the skip path and the normal resolve-before-push path

## Out of Scope

- Changing how `resolve_command` itself works
- Changing pull/fetch semantics outside the pre-push resolve decision
- Optimizing other server-side git operations unrelated to `git/sync`
