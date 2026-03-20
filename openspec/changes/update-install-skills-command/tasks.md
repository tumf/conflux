## Implementation Tasks

- [x] Update CLI parsing in `src/cli.rs` so `install-skills` accepts only optional `--global` scope selection and rejects positional source arguments (verification: unit tests in `src/cli.rs` cover `cflx install-skills`, `cflx install-skills --global`, and legacy extra-argument failures).
- [x] Simplify `src/install_skills.rs` to always install from the bundled repository `skills/` directory while preserving project/global target resolution (verification: `tests/install_skills_test.rs` covers project and global bundled installs without a source string).
- [x] Add regression coverage for legacy invocations such as `cflx install-skills self` and `cflx install-skills local:...` returning migration guidance (verification: targeted CLI or integration tests assert the error text mentions `cflx install-skills [--global]`).
- [x] Update `README.md` and any related command documentation to describe the new syntax and scope behavior (verification: `README.md` examples only show `cflx install-skills` and `cflx install-skills --global`).
- [x] Run Rust verification for the changed command path (verification: `cargo test install_skills` and any focused CLI tests pass).

## Future Work

- Reintroduce configurable source selection in a separate proposal only if a real non-bundled workflow emerges.
