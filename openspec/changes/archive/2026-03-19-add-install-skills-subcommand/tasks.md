## Implementation Tasks

- [x] Add the `agent-skills-rs` dependency and define the bundled `skills/` source layout for `self` installs (verification: `Cargo.toml`, new top-level `skills/` contents, and compile references resolve without placeholder imports).
- [x] Add CLI parsing for `cflx install-skills <source> [--global]` in `src/cli.rs` with tests for `self`, `local:<path>`, and unsupported schemes (verification: unit tests in `src/cli.rs` cover accepted and rejected inputs).
- [x] Implement command dispatch in `src/main.rs` and a dedicated install flow module that resolves project/global destinations and matching lock-file paths (verification: source wiring in `src/main.rs` plus destination resolution logic in the new module references `.agents/skills` and `.agents/.skill-lock.json` consistently).
- [x] Integrate `agent-skills-rs` discovery and install APIs for `self` and `local:<path>` sources, including explicit error messages for unsupported schemes (verification: implementation uses `SourceType::Self_` or local source construction and returns deterministic errors for invalid schemes).
- [x] Add integration or filesystem-focused tests that verify project-scope and global-scope installs write to the expected directories and update the matching lock file (verification: test files under `tests/` or module tests assert install paths and lock paths for both scopes).
- [x] Update user-facing documentation for the new command and bundled skills layout (verification: README or relevant docs mention `cflx install-skills self` and `local:<path>` with expected destination behavior).

## Future Work

- Consider command introspection output for machine-readable command discovery if Conflux later adopts a schema or commands API similar to `agent-exec`.
- Consider follow-up commands for uninstall, update, or listing installed skills.

## Acceptance #1 Failure Follow-up

- [x] Remove `.agents/.skill-lock.json` from this change (or replace it with deterministic, non-environment-dependent fixture data) so the change does not commit absolute temp paths or timestamp-dependent test artifacts.
