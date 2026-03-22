## Implementation Tasks

- [x] 1.1 Bump `agent-skills-rs` dependency in `Cargo.toml` to the version with `register_embedded_skill()` and `auxiliary_files` support (verification: `cargo build` succeeds)
- [x] 1.2 Create `src/embedded_skills.rs` module that uses `include_str!` to embed all three skills (cflx-proposal, cflx-workflow, cflx-run) with their auxiliary files via `register_embedded_skill()` (verification: new unit test `test_embedded_skills_count` asserts 3 skills returned)
- [x] 1.3 Register `embedded_skills` module in `src/lib.rs` (verification: `cargo build` succeeds)
- [x] 1.4 Update `src/install_skills.rs` to prefer embedded skills via `get_cflx_embedded_skills()` and fall back to `SourceType::Local` + `skills/` only when `skills/` dir exists at project_root (verification: `cargo test install_skills` passes)
- [x] 1.5 Update `src/install_skills.rs` to use `SourceType::Self_` source when installing from embedded (verification: lock file entry has `source_type: "self"`)
- [x] 1.6 Add integration test in `tests/install_skills_test.rs`: `test_embedded_install_without_skills_dir` — creates a tempdir with NO `skills/` directory and verifies that `run_install_skills` succeeds with 3 skills installed including auxiliary files (verification: `cargo test test_embedded_install_without_skills_dir`)
- [x] 1.7 Verify each embedded skill installs complete file tree: SKILL.md plus `scripts/cflx.py` and `references/*.md` where applicable (verification: assertion in test from 1.6 checks file existence)
- [x] 1.8 Ensure existing `tests/install_skills_test.rs` tests pass without modification — they exercise the dev fallback path via `project_root` + `skills/` (verification: `cargo test --test install_skills_test`)
- [x] 1.9 Run full test suite and clippy (verification: `cargo test && cargo clippy -- -D warnings`)

## Acceptance #1 Failure Follow-up

- [x] Make `run_install_skills` prefer embedded skills first, and fall back to local `skills/` discovery only when embedded skills are unavailable.
- [x] Update `install-skills` CLI long help text to describe embedded-by-default behavior and conditional local fallback.
- [x] Add a regression test that verifies embedded skills are selected even when a local `skills/` directory exists.

## Acceptance #2 Failure Follow-up

- [ ] Clean the working tree before rerunning acceptance (currently modified: `tests/install_skills_test.rs`).

## Future Work

- Embed `.skill` binary manifest files once binary embedding is supported in `agent-skills-rs`
- Remove dev fallback path if embedded-only distribution is confirmed sufficient
- Consider `build.rs` auto-discovery to reduce manual `include_str!` maintenance
- Ensure the working tree is clean before rerunning acceptance (human step: `git status` check before acceptance run)
