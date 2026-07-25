---
change_type: implementation
priority: medium
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - src/cli.rs
  - src/main.rs
  - https://herdr.dev/docs/plugins/
  - https://herdr.dev/docs/cli-reference/
verifications:
  - id: herdr-plugin-local-verification
    requirement: The bundled Herdr plugin launches cflx tui in a managed pane and reports the pane as agent cflx
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: cargo test output plus local plugin fixture assertions recorded in the acceptance review
    rerun: make test
    prerequisites: []
---

# Add Herdr cflx Agent Pane

**Change Type**: implementation

## Problem / Context

Herdr can launch plugin-owned terminal panes, but Conflux does not provide a Herdr plugin entrypoint for `cflx tui`. A user can run the TUI manually, yet the pane is not deliberately registered in Herdr's Agents list under the requested label `cflx`.

The integration must remain observational. Herdr state must not become an authoritative input for Conflux resume, acceptance, archive, or next-action decisions, in accordance with `openspec/CONSTITUTION.md`.

## Proposed Solution

Add a minimal Herdr plugin package to this repository with:

- plugin id `tumf.cflx`;
- a managed pane entrypoint named `tui` that runs `cflx tui` in the selected Herdr workspace cwd;
- a small launcher that reports the managed pane to Herdr with agent label `cflx` before replacing itself with the TUI process;
- cleanup that releases the `cflx` lifecycle authority when the TUI exits or launch fails;
- macOS and Linux support with no runtime dependency beyond a POSIX shell, `cflx`, and the Herdr CLI supplied through `HERDR_BIN_PATH`.

Keep this as an external executable plugin package rather than adding a plugin runtime to the Conflux binary.

## Acceptance Criteria

- A user can link or install the repository's Herdr plugin and open its `tui` pane entrypoint from a Conflux workspace.
- The pane starts `cflx tui` with the Herdr workspace cwd unchanged, so project-local `.cflx.jsonc` and `openspec/` discovery keep their existing behavior.
- While the TUI process is active, Herdr's Agents list identifies that pane with the exact agent label `cflx`.
- When `cflx tui` exits or cannot start, the plugin releases its Herdr agent report and preserves the TUI process exit status.
- Missing `HERDR_PANE_ID`, `HERDR_BIN_PATH`, or `cflx` produces a concise non-zero failure instead of registering stale agent state.
- Herdr-reported state is not read by Conflux as workflow-control input.
- The plugin manifest and launcher behavior are covered by repository-verifiable automated tests, including success and launch-failure cleanup paths.

## Explicit Completion Conditions

- The repository contains a valid `herdr-plugin.toml` and executable launcher under one focused plugin directory.
- Tests verify the manifest identity, `tui` pane command, exact `cflx` label, cwd preservation, exit-code forwarding, and release behavior with fake Herdr and cflx executables.
- Installation/link usage and the command for opening the pane are documented next to existing user-facing integration documentation.
- `cflx openspec validate add-herdr-cflx-agent-pane --strict --evidence warn`, formatting, lint, and the relevant test suite pass.

## Out of Scope

- Adding a generic plugin system to Conflux.
- Mapping internal Conflux phases to Herdr `idle`, `blocked`, or richer status labels.
- Restoring a prior `cflx tui` session after Herdr restart.
- Windows support.
- Using Herdr state to influence Conflux workflow decisions.
- Publishing the plugin repository or adding the GitHub marketplace topic as part of this change.
