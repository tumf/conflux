# Change: Treat Byte Reception as Activity for Inactivity Timeout

## Problem/Context

Conflux currently treats "activity" during streaming command execution as receiving stdout/stderr *lines*.
This can misclassify commands as inactive when output is buffered or does not contain newlines (e.g. progress dots, carriage-return updates, or long-running test runs that only print a final summary).

The observed symptom is frequent runs where no log lines are emitted for ~15 minutes, an inactivity timeout is reported, and then output appears shortly after.

## Proposed Solution

- Redefine inactivity-timeout activity detection to be based on *byte reception* from stdout/stderr, not line reception.
- Keep log emission line-oriented (emit full lines when `\n` is observed), but update `last_activity` whenever any bytes are read from stdout or stderr.
- Count both stdout and stderr byte reception as activity.

## Acceptance Criteria

- If a command emits bytes periodically but does not emit newline-terminated lines, inactivity timeout MUST NOT trigger.
- If a command emits bytes periodically on stderr (and not stdout), inactivity timeout MUST NOT trigger.
- If a command emits no bytes on stdout/stderr for the configured timeout window, inactivity timeout MUST trigger (unchanged behavior).
- Existing human-facing log formatting remains line-oriented.

## Out of Scope

- Changing the default inactivity timeout duration (e.g. 900s) or the kill grace behavior.
- Adding CPU-based or filesystem-based activity detection.
- Adding a global max wall-clock runtime limit.
