# Change: Merge Stall Circuit Breaker

## Why
When merge commits to the base branch are not reflected for an extended period, orchestration may be effectively stalled without being noticed. By explicitly detecting stalls (no merge progress for 30+ minutes), the system can stop immediately and facilitate recovery decisions.

## What Changes
- Monitor merge commit progress to the base branch and immediately halt orchestration if no progress occurs for 30 minutes
- Monitoring applies **only to parallel mode** (serial mode does not create `Merge change:` commits, so it is out of scope)
- Reflect the stop reason in CLI/TUI/Web events and logs
- Make monitoring interval and threshold configurable

## Architectural Constraint
- Serial mode does not use worktree isolation and does not create `Merge change: <change_id>` commits
- Merge stall monitoring relies on the timestamp of `Merge change:` commits, so it is a parallel-mode-only feature
- Serial mode continues to use existing error detection circuit breakers

## Impact
- Affected specs: circuit-breaker, configuration, parallel-execution, cli, tui-architecture, web-monitoring
- Affected code: orchestrator run loop, parallel executor run loop, config parsing, web state updates
