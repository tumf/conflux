---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/config/types.rs
  - src/config/defaults.rs
  - src/log_viewer.rs
  - docs/guides/CONFIG.md
verifications:
  - id: configurable-state-root-unit-tests
    requirement: Configuration parsing, merge, normalization, path validation, precedence, and child-environment preservation are repository-locally verified
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: src/config/defaults.rs
    evidence: cargo test config --lib
    rerun: cargo test config --lib
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: configurable-state-root-logs-tests
    requirement: The cflx logs command reads the same configured state root used by logging and remains side-effect free
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/logs_command_tests.rs
    evidence: cargo test --test logs_command_tests
    rerun: cargo test --test logs_command_tests
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: configurable-state-root-startup-tests
    requirement: Invalid configured roots fail before lifecycle or AI child commands start at each orchestration entrypoint
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: tests/run_exit_tests.rs
    evidence: cargo test --test run_exit_tests configurable_state_root
    rerun: cargo test --test run_exit_tests configurable_state_root
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: configurable-state-root-doc-check
    requirement: Configuration documentation and the generated init template remain syntactically clean after documenting both storage roots
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: docs/guides/CONFIG.md
    evidence: git diff --check -- docs/guides/CONFIG.md src/main.rs
    rerun: git diff --check -- docs/guides/CONFIG.md src/main.rs
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Configure CFLX-owned storage without changing process-wide XDG paths

**Change Type**: implementation

## Problem / Context

Conflux currently supports `workspace_base_dir` for managed worktrees, but persistent logs are rooted only through `XDG_STATE_HOME` or the platform fallback `~/.local/state`.

Operators who place large Conflux data on an external disk can configure worktrees directly, but cannot configure the Conflux log/state root without changing `XDG_STATE_HOME` for the entire Conflux process tree or installing filesystem symlinks.

Changing `XDG_STATE_HOME` is too broad because Apply, Acceptance, Archive, Resolve, and lifecycle child processes inherit it. Their unrelated XDG state may then move with Conflux logs.

## Proposed Solution

Add an optional `state_base_dir` field to `config.jsonc`.

- When set, Conflux stores its non-authoritative persistent state under `<state_base_dir>/cflx/`, including logs at `<state_base_dir>/cflx/logs/<project_slug>/`.
- Configuration precedence is `state_base_dir`, then `XDG_STATE_HOME`, then the existing platform fallback.
- The setting affects only Conflux-owned paths. It does not mutate or inject `XDG_STATE_HOME` into child processes.
- Empty values behave as unset, matching `workspace_base_dir`.
- A configured path must be absolute. Startup fails before orchestration begins if the root is unavailable, cannot be created, or is not writable. Conflux must not silently fall back to the internal disk.
- `cflx logs`, retention cleanup, and normal logging must resolve the same configured root.
- Both orchestration entrypoints load and validate configuration before logging initialization. Configuration-load failures therefore remain stderr diagnostics rather than being written to a configured log file.
- Document the existing `workspace_base_dir` together with the new `state_base_dir`, including an external-volume example.

Example:

```jsonc
{
  "workspace_base_dir": "/Volumes/OWCUS4EXP1M2/mini-data/cflx/worktrees",
  "state_base_dir": "/Volumes/OWCUS4EXP1M2/mini-data/cflx/state"
}
```

## Acceptance Criteria

- `workspace_base_dir` continues to select the managed-worktree root without regression.
- `state_base_dir` selects the Conflux-owned persistent state root for logging, retention cleanup, and `cflx logs`.
- `state_base_dir` takes precedence over `XDG_STATE_HOME` without altering the environment inherited by child commands.
- Relative, unavailable, and unwritable configured paths produce an actionable startup error before an owner, lifecycle adapter, or AI command starts.
- An absent or empty `state_base_dir` preserves current XDG and platform-default behavior.
- Documentation contains a copy-pasteable external-volume configuration example and explains that existing files are neither migrated nor cleaned up automatically after the root changes. `cflx logs` lists projects only from the currently resolved root.

## Explicit Completion Conditions

- Configuration parsing, merge behavior, empty-value normalization, and precedence are covered by focused Rust tests.
- Logging initialization, cleanup, and `cflx logs` use one shared path resolver.
- A focused startup test proves an invalid configured root fails before child-process launch.
- Existing default-path tests remain green.
- `docs/guides/CONFIG.md` and the generated/init configuration template describe both storage settings and migration responsibility.

## Out of Scope

- Automatically moving existing worktrees or logs.
- Making external logs authoritative workflow state.
- Changing the workspace-local workflow-state law in `openspec/CONSTITUTION.md`.
- Setting process-wide `XDG_DATA_HOME` or `XDG_STATE_HOME` for child commands.
- Supporting relative paths or shell expansion in configured storage roots.
