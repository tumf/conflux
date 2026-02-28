# Change: Retry Commands After Inactivity Timeout (Configurable)

## Problem / Context

Conflux terminates streaming commands when they produce no stdout/stderr for a configured interval (inactivity timeout).

This is useful to prevent indefinite hangs, but in real-world usage the underlying cause can be intermittent (LLM/API stall, network hiccup, pipeline buffering, etc.). In these cases, immediately failing the change after a single timeout can be unnecessarily disruptive: a simple retry often succeeds.

Today, inactivity-timeout-triggered terminations are treated as non-retryable, even when command retry is enabled for other transient failures.

## Proposed Solution

Add a dedicated, opt-in retry policy for inactivity timeout.

- Introduce configuration to retry a command when it is terminated due to inactivity timeout.
- Keep defaults safe-by-default: inactivity-timeout retry disabled unless explicitly configured.
- Make the number of inactivity-timeout retries configurable. This request targets **3 retries**.
- Emit explicit user-facing output lines and logs indicating the retry reason is `inactivity timeout` (distinct from crash/pattern retries).

## Acceptance Criteria

- When `command_inactivity_timeout_max_retries` is set to `3`, a command terminated due to inactivity timeout is automatically retried up to 3 times.
- Each retry is delayed by the existing `command_queue_retry_delay_ms`.
- Retry notifications are visible in streaming output (stderr) and include attempt counts and the inactivity-timeout reason.
- After exhausting inactivity-timeout retries, the final error message includes:
  - `inactivity timeout`
  - total retries attempted
  - operation/change context when available
- Existing retry behavior for other failure modes (pattern/short duration/crash) remains unchanged.

## Out of Scope

- Changing the default inactivity timeout duration (still default 900s).
- Automatically increasing the inactivity timeout on retries.
- Replacing the underlying agent command runner (`cc-stream`, `cc-stream-filter`, etc.).

## Impact

- Affected specs: `openspec/specs/command-queue/spec.md`, `openspec/specs/configuration/spec.md`
- Likely affected code:
  - `src/ai_command_runner.rs`
  - `src/command_queue.rs`
  - `src/config/mod.rs`
  - `src/config/defaults.rs`
