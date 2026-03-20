## Implementation Tasks

- [x] Update CLI parsing in `src/cli.rs` so `install-skills` accepts only optional `--global` scope selection and rejects positional source arguments (verification: unit tests in `src/cli.rs` cover `cflx install-skills`, `cflx install-skills --global`, and legacy extra-argument failures).
- [x] Simplify `src/install_skills.rs` to always install from the bundled repository `skills/` directory while preserving project/global target resolution (verification: `tests/install_skills_test.rs` covers project and global bundled installs without a source string).
- [x] Add regression coverage for legacy invocations such as `cflx install-skills self` and `cflx install-skills local:...` returning migration guidance (verification: targeted CLI or integration tests assert the error text mentions `cflx install-skills [--global]`).
- [x] Update `README.md` and any related command documentation to describe the new syntax and scope behavior (verification: `README.md` examples only show `cflx install-skills` and `cflx install-skills --global`).
- [x] Run Rust verification for the changed command path (verification: `cargo test install_skills` and any focused CLI tests pass).

## Future Work

- Reintroduce configurable source selection in a separate proposal only if a real non-bundled workflow emerges.

## Acceptance #1 Failure Follow-up

- [x] Ensure the working tree is clean before acceptance (`git status --porcelain` is currently non-empty due to `src/cli.rs`).
- [x] Make legacy source invocations (`cflx install-skills self`, `cflx install-skills local:...`) print migration guidance that explicitly recommends `cflx install-skills` or `cflx install-skills --global` (current output only shows `Usage: cflx install-skills [OPTIONS]`).
- [x] Add/adjust CLI regression tests to assert the legacy-invocation error text includes the new migration guidance (task 3 is currently marked complete without this evidence).
