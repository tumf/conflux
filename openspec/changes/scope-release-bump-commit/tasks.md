## Implementation Tasks

- [ ] Define one explicit main/master release-owned path array in `scripts/bump.sh` containing `Cargo.toml`, `Cargo.lock`, and existing `docs/openapi.yaml`; reject staged or unstaged changes on those paths before mutation while allowing unrelated dirty state. (verification: integration - `cargo test --test release_bump_scope_tests` proves owned-dirty rejection occurs before side effects and unrelated dirtiness remains allowed; verification-id: scoped-release-bump-tests)

- [ ] Reuse the owned-path array for generated-delta checks, scoped staging, and `git commit --only -- <owned-paths>` or equivalent index isolation, preserving unrelated staged, unstaged, and untracked state plus `OPENSPEC_GIT_COMMIT_NO_VERIFY`. (verification: integration - `cargo test --test release_bump_scope_tests` inspects the real commit diff and before/after index/worktree state, including a pre-staged unrelated file; verification-id: scoped-release-bump-tests)

- [ ] Make no-delta and pre-commit failure paths exit before tag and push without cleaning local state; ensure a retry with dirty owned paths fails without calculating or creating a later version. (verification: integration - `cargo test --test release_bump_scope_tests` injects generation/stage/commit failures and asserts refs, push evidence, retained state, and retry behavior; verification-id: scoped-release-bump-tests)

- [ ] Add repository-visible recovery for a valid current-version release commit missing its tag and for a current-version tag already pointing to `HEAD`, completing tag/push for the same version without another bump. Keep dry-run side-effect-free in both recovery states. (verification: integration - `cargo test --test release_bump_scope_tests` injects tag and push failures against a local bare origin, reruns, and proves same-version completion with no extra commit or tag; verification-id: scoped-release-bump-tests)

- [ ] Preserve main/master version calculation, annotated tag format, current push behavior, and leave the non-main `cargo release` delegation unchanged. (verification: integration - `cargo test --test release_bump_scope_tests` covers patch/minor/major calculation and verifies feature-branch execution still delegates unchanged; verification-id: scoped-release-bump-tests)

- [ ] Update `docs/guides/RELEASE.md` to describe clean release-owned paths, permitted unrelated dirty work, scoped commit behavior, dry-run safety, and recovery after commit/tag/push partial completion. (verification: integration - `cargo test --test release_bump_scope_tests` checks the tracked guide contains the owned-path and retry contract used by the tested workflow; verification-id: scoped-release-bump-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate scope-release-bump-commit --archive-gate`

The implementation must also pass `cargo test --test release_bump_scope_tests`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings`.

## Future Work

- A separate change may introduce repository-wide cross-process mutation coordination if scoped ownership and read-only lock suppression prove insufficient.
- Push refspec narrowing or atomic branch/tag publication may be proposed separately.
