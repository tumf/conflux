# Change: Enforce --no-verify prohibition in apply prompt

## Why
The apply system prompt should explicitly forbid use of `--no-verify` to avoid bypassing hooks during automated changes.

## What Changes
- Add a prompt instruction that prohibits `--no-verify` when running git commands in apply mode.

## Impact
- Affected specs: `specs/agent-prompts/spec.md`
- Affected code: None (prompt instruction change only)
