## Implementation Tasks

- [ ] Define one explicit release-owned artifact path set in `scripts/bump.sh` and use it for owned-delta detection, staging, and release commit creation on main/master without repository-wide `git add -A`. (verification: integration - `cargo test --test release_bump_scope_tests` inspects a real temporary Git commit and proves its diff is restricted to owned artifacts; verification-id: scoped-release-bump-tests)

- [ ] Preserve unrelated staged, unstaged, and untracked files across a successful release bump, including their index/worktree distinction, while retaining branch-aware versioning, `OPENSPEC_GIT_COMMIT_NO_VERIFY`, annotated tags, and push invocation. (verification: integration - `cargo test --test release_bump_scope_tests` snapshots porcelain/index state before and after the controlled release and fails on unrelated mutation or inclusion; verification-id: scoped-release-bump-tests)

- [ ] Make no-op and already-released decisions depend only on release-owned paths so unrelated dirty work cannot create a release commit or defeat idempotent completion detection. (verification: integration - `cargo test --test release_bump_scope_tests` covers an already-tagged HEAD plus unrelated dirty files and an unchanged owned-artifact case; verification-id: scoped-release-bump-tests)

- [ ] Add failure-path coverage proving a scoped stage or commit error exits before annotated tag creation and push, without cleaning or committing unrelated work. (verification: integration - `cargo test --test release_bump_scope_tests` injects a controlled Git failure and asserts absent release refs/push evidence plus preserved unrelated state; verification-id: scoped-release-bump-tests)

- [ ] Apply the owned-path boundary to the non-main pre-release path if code inspection shows it performs repository-wide staging, while preserving its SemVer branch suffix behavior. (verification: integration - `cargo test --test release_bump_scope_tests` covers a feature-branch bump and asserts both the suffix and scoped commit contents; verification-id: scoped-release-bump-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate scope-release-bump-commit --archive-gate`

The implementation must also pass `cargo test --test release_bump_scope_tests`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings`.

## Future Work

- A separate change may introduce repository-wide cross-process mutation coordination if scoped ownership and read-only lock suppression prove insufficient for other workflows.
