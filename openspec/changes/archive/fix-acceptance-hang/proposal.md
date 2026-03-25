# Change: Fix acceptance phase hanging when agent process does not exit

## Why

The acceptance phase hangs indefinitely after the AI agent outputs `ACCEPTANCE: PASS` (or other verdicts). This is caused by three compounding issues: (1) the verdict parser uses exact string matching which fails when the agent concatenates trailing text on the same line, (2) the output streaming loop blocks forever when the agent process does not terminate because child processes (e.g., MCP servers) keep stdout/stderr pipes open, and (3) skill/command definitions lack explicit instructions requiring the verdict marker to be on its own line.

## What Changes

- Verdict parsing uses prefix matching (`starts_with`) instead of exact match (`==`) so that trailing text on the marker line does not prevent detection
- Acceptance output streaming applies a 30-second grace period after detecting a verdict marker; if the agent process does not exit within that window it is terminated
- Skill and command definitions for acceptance include a CRITICAL formatting rule requiring the verdict marker to be on its own line

## Impact

- Affected specs: `cli` (orchestration loop acceptance verdict parsing and grace period), `agent-prompts` (acceptance marker formatting rule)
- Affected code: `src/acceptance.rs`, `src/orchestration/acceptance.rs`, `skills/cflx-workflow/SKILL.md`, `skills/cflx-workflow/references/cflx-accept.md`, `.opencode/commands/cflx-accept.md`
