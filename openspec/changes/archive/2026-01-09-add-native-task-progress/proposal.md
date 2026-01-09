# Change: Add Native Task Progress Parsing

## Why

The current implementation relies on `openspec list --json` to get task progress (`completedTasks`/`totalTasks`). However, OpenSpec CLI has a bug where numbered task lists (e.g., `1. [x] Task`) are not recognized - only bullet lists (`- [x] Task`) work. This is a known issue ([OpenSpec Issue #354](https://github.com/Fission-AI/OpenSpec/issues/354)).

To make openspec-orchestrator independent of this upstream bug and provide more reliable task progress information, we should implement native `tasks.md` parsing in Rust.

## What Changes

- Add native `tasks.md` parsing module that directly reads and parses task files
- Parse both bullet lists (`- [ ]`, `- [x]`) and numbered lists (`1. [ ]`, `1. [x]`)
- Update `list_changes` to use native parsing when `openspec list --json` returns 0/0 task counts
- Add configuration option to prefer native parsing over openspec CLI output

## Impact

- Affected specs: `cli`, `configuration`
- Affected code: `src/openspec.rs`, new `src/task_parser.rs` module
- No breaking changes - falls back to openspec CLI if tasks.md not found
