# Change: Add hook git commit no-verify option

## Problem / Context
`on_merged` is currently configured in this repository as a best-effort release hook (`make bump-patch`). In practice, the release path can invoke git commit flows that trigger repository commit hooks such as formatting, clippy, and OpenAPI generation. That extra work can cause the hook to exceed its configured timeout, leaving the release incomplete and changes uncommitted.

Today hook configuration already supports both string and object forms, but the object form only exposes `command`, `timeout`, and `continue_on_failure`. There is no standard way for a hook configuration to request that downstream git commit operations skip commit verification hooks when the hook is intentionally running trusted automation.

## Proposed Solution
- Extend detailed hook object configuration with a new boolean option `git_commit_no_verify`
- Keep string-form hook configuration fully backward compatible
- Default `git_commit_no_verify` to `false`
- Surface the option from hook configuration into the hook execution environment so release workflows can opt into `--no-verify` behavior without encoding that policy directly into every hook command string
- Document that this option is intended for trusted automation hooks such as release workflows, especially `on_merged`

## Acceptance Criteria
- Hook configuration continues to support both simple string form and detailed object form
- Detailed hook configuration supports `git_commit_no_verify: true|false`
- When omitted, `git_commit_no_verify` behaves as `false`
- Hook execution exposes the option to child commands in a machine-readable way so release scripts/wrappers can decide whether to use `git commit --no-verify`
- Existing repositories using string-form hook configuration continue to work unchanged
- Templates remain string-form examples unless explicitly changed by a separate proposal

## Out of Scope
- Changing release workflow behavior by itself
- Forcing all hooks to bypass git verification
- Redesigning cargo-release or git hook internals beyond the config surface and hook-to-command propagation needed for this option
