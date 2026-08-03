## Implementation Tasks

- [x] Define one explicit main/master release-owned path array in `scripts/bump.sh` containing `Cargo.toml`, `Cargo.lock`, and existing `docs/openapi.yaml`; reject staged or unstaged changes on those paths before mutation while allowing unrelated dirty state. (verification: integration - `cargo test --test release_bump_scope_tests` proves owned-dirty rejection occurs before side effects and unrelated dirtiness remains allowed; verification-id: scoped-release-bump-tests)

- [x] Reuse the owned-path array for generated-delta checks, scoped staging, and `git commit --only -- <owned-paths>` or equivalent index isolation, preserving unrelated staged, unstaged, and untracked state plus `OPENSPEC_GIT_COMMIT_NO_VERIFY`. (verification: integration - `cargo test --test release_bump_scope_tests` inspects the real commit diff and before/after index/worktree state, including a pre-staged unrelated file; verification-id: scoped-release-bump-tests)

- [x] Make no-delta and pre-commit failure paths exit before tag and push without cleaning local state; ensure a retry with dirty owned paths fails without calculating or creating a later version. (verification: integration - `cargo test --test release_bump_scope_tests` injects generation/stage/commit failures and asserts refs, push evidence, retained state, and retry behavior; verification-id: scoped-release-bump-tests)

- [x] Add repository-visible recovery for a valid current-version release commit missing its tag and for a current-version tag already pointing to `HEAD`, completing tag/push for the same version without another bump. Keep dry-run side-effect-free in both recovery states. (verification: integration - `cargo test --test release_bump_scope_tests` injects tag and push failures against a local bare origin, reruns, and proves same-version completion with no extra commit or tag; verification-id: scoped-release-bump-tests)

- [x] Preserve main/master version calculation, annotated tag format, current push behavior, and leave the non-main `cargo release` delegation unchanged. (verification: integration - `cargo test --test release_bump_scope_tests` covers patch/minor/major calculation and verifies feature-branch execution still delegates unchanged; verification-id: scoped-release-bump-tests)

- [x] Update `docs/guides/RELEASE.md` to describe clean release-owned paths, permitted unrelated dirty work, scoped commit behavior, dry-run safety, and recovery after commit/tag/push partial completion. (verification: integration - `cargo test --test release_bump_scope_tests` checks the tracked guide contains the owned-path and retry contract used by the tested workflow; verification-id: scoped-release-bump-tests)

## Final Validation

Archive validation itself is the authoritative final OpenSpec validation gate.
Expected archive gate: `cflx openspec validate scope-release-bump-commit --archive-gate`

The implementation must also pass `cargo test --test release_bump_scope_tests`, `cargo fmt -- --check`, and `cargo clippy -- -D warnings`.

Results on 2026-08-03:
- `cargo test --test release_bump_scope_tests` -> ok, 26 passed, 0 failed.
- `cargo fmt -- --check` -> clean.
- `cargo clippy --all-targets -- -D warnings` -> finished with no warnings.

## Notes

- Verification type: integration (verification-id `scoped-release-bump-tests`). Every task in this change is verified by `tests/release_bump_scope_tests.rs`, which drives the real `scripts/bump.sh` against temporary Git repositories, a local bare origin, real Git hooks (`pre-commit`, `pre-push`, `reference-transaction`), and a controlled fake `cargo` on `PATH`. No network access is used. This matches the planned verification path in the proposal; no unit-scoped evidence is claimed.
- `docs/guides/RELEASE.md` is updated alongside the two `scripts/bump.sh` findings so the documented contract still matches the script: owned-path ownership now survives deletion of a tracked `docs/openapi.yaml`, and a resume requires commit contents plus an annotated tag rather than a matching subject. `release_guide_documents_the_owned_path_contract` asserts both statements.
- Staging failure is injected by holding `.git/index.lock`, which is the real contention failure this scoping change was written for: `git add -- <owned paths>` fails at the Git level, before any commit, tag, or push. Ownership of `docs/openapi.yaml` is decided from worktree, index, and `HEAD` state so a deleted tracked artifact is still validated, and resume states are accepted only on commit contents plus annotated-tag object type rather than on the commit subject alone.
- Tag-failure and push-failure states are injected with real Git hooks rather than constructed by hand, so the recovery tests observe the same repository-visible evidence the script uses. The `reference-transaction` hook requires git >= 2.28; the test asserts the expected post-failure state explicitly so an older git fails loudly instead of silently skipping.
- Test-speed note vs `AGENTS.md`: these tests are kept in the default tier because the proposal declares `cargo test --test release_bump_scope_tests` as the change-blocking rerun, and gating them behind `heavy-tests` would make that command skip all evidence. Per-test spawn count was reduced (git identity via environment instead of `git config` calls, one `git status --porcelain` instead of three queries, one `git show-ref` instead of per-ref lookups, and the minor/major case split into single-bump tests). Measured best-of-3 per-test wall time ranged 0.3s-2.4s only while this shared machine was running other cargo builds (load average 10-21, measured at ~19ms per process spawn versus ~7-8ms unloaded); the whole file completes in about 5-8s wall when contention is low.

## Future Work

- A separate change may introduce repository-wide cross-process mutation coordination if scoped ownership and read-only lock suppression prove insufficient.
- Push refspec narrowing or atomic branch/tag publication may be proposed separately.

## Current Acceptance Follow-up
- attempt: 1
- [x] `openspec/changes/scope-release-bump-commit/tasks.md` (lines 7 and 29) claims staging-failure integration evidence, but `tests/release_bump_scope_tests.rs` injects generation and commit failures only; inject a real `git add` failure and assert no tag/push and no version-advancing retry.
  evidence: `staging_failure_blocks_tag_push_and_a_version_advancing_retry` holds `.git/index.lock` so the real `git add -- <owned paths>` fails, and asserts no commit, no tag, unchanged origin refs, and a retry that stops at dirty owned paths without advancing past 0.1.1; tasks.md notes now describe that injection.
- [x] Recovery in `scripts/bump.sh` (lines 140-167) treats a matching subject as sufficient proof of a valid release commit and accepts a lightweight `vX.Y.Z` tag at HEAD, violating scoped-release-commit and annotated-tag requirements; validate commit contents/tag object type and add integration cases for both invalid states.
  evidence: `scripts/bump.sh` adds `head_is_scoped_release_commit` (subject plus a non-empty change set limited to owned paths and including `Cargo.toml`) and `tag_is_annotated` gating both recovery branches, covered by `release_labelled_commit_with_unrelated_contents_is_not_resumed` and `lightweight_tag_at_head_is_rejected_instead_of_published`.
- [x] `scripts/bump.sh:set_release_owned_paths` (lines 74-79) excludes a tracked `docs/openapi.yaml` after it is deleted from the worktree, allowing a dirty release-owned artifact to bypass pre-mutation rejection; determine ownership from tracked/HEAD state and add an integration test proving deletion creates no commit, tag, or push.
  evidence: `set_release_owned_paths` now claims `docs/openapi.yaml` when it exists in the worktree, in the index (`git ls-files --error-unmatch`), or in `HEAD` (`git cat-file -e`), proven by `deleted_tracked_openapi_blocks_release` and `staged_deletion_of_tracked_openapi_blocks_release`.
