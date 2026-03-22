# Change: Embed bundled skills into cflx binary

## Problem/Context

- `cflx install-skills` claims to install "bundled Conflux agent skills" (spec: `openspec/specs/cli/spec.md`, README).
- In reality, `src/install_skills.rs` uses `SourceType::Local` pointing at `{CWD}/skills/` — a runtime filesystem dependency.
- `Cargo.toml` `exclude` contains `skills/**`, so `cargo publish` / `cargo install` produces a binary with no access to the `skills/` directory.
- Running `cflx install-skills` outside the source repository yields `No skills found in skills/ directory.`
- `agent-skills-rs` (dependency, v0.3.0) already provides an embedded skill mechanism (`SourceType::Self_`, `get_embedded_skills()`), but cflx does not use it.
- A companion proposal (`add-multi-file-embedded-skills` in `agent-skills-rs`) extends that library to support auxiliary files (scripts, references) alongside `SKILL.md`.

## Proposed Solution

1. **Register cflx skills as embedded** — Use `include_str!` in a new `src/embedded_skills.rs` module to embed all three skills (cflx-proposal, cflx-workflow, cflx-run) with their auxiliary files (scripts, references) at compile time via `agent-skills-rs`'s `register_embedded_skill()` API.

2. **Switch `install_skills.rs` to use embedded source** — Replace the current `SourceType::Local` + filesystem discovery with `SourceType::Self_` using the embedded skill registry. The function `run_install_skills()` calls the embedded skill list directly instead of `discover_skills()` from a local path.

3. **Dev fallback** — When embedded skills are empty (e.g. during library-only builds or testing without embed), fall back to the current `SourceType::Local` + `skills/` directory discovery for development convenience.

4. **Bump `agent-skills-rs` dependency** — Update `Cargo.toml` to the version that includes multi-file embedded skill support.

## Acceptance Criteria

- `cargo install cflx` produces a binary that successfully runs `cflx install-skills` in any directory (no `skills/` needed).
- All three skills (cflx-proposal, cflx-workflow, cflx-run) are installed with their full file trees (SKILL.md, scripts/, references/).
- `cflx install-skills --global` works identically with global scope.
- Lock file `source_type` is recorded as `"self"`.
- Existing integration tests (`tests/install_skills_test.rs`) continue to pass (they use `project_root` with a test `skills/` dir — this exercises the dev fallback path).
- New test verifies that the embedded skills are discoverable and contain expected auxiliary files.

## Out of Scope

- Embedding `.skill` binary manifest files (text-only for now).
- Removing the `skills/` directory from the repository (still needed for development and for the `agent-skills-rs` local discovery path).
- Changes to the `Cargo.toml` `exclude` list (embedded content is resolved at build time regardless of exclude).
