# Proposal: increase-log-limit

## Summary

Increase the TUI log entry limit from 100 to 1000 entries.

## Background

The current log limit of 100 entries is insufficient for long-running orchestration sessions. Users want to review more historical log entries, especially when debugging issues.

## Scope

- Increase `MAX_LOG_ENTRIES` constant from 100 to 1000

## Impact

- Memory usage will increase slightly (approximately 10x more log entries stored)
- Each `LogEntry` contains: timestamp (String), message (String), color (enum)
- Estimated memory increase: ~100KB to ~1MB depending on log message lengths
- This is acceptable for a TUI application

## Out of Scope

- Configurable log limit (YAGNI for now)
- Log persistence to file
- Log filtering
