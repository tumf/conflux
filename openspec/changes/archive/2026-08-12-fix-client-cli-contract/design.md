# Design: bounded client contract corrections

## JSON parse boundary

The executable must use Clap's non-exiting parse result so it can inspect parse failures. Classification is intentionally narrow: only argv that selects the `client` namespace and includes the exact `--json` flag receives the stable JSON `usage_error` envelope. Do not infer JSON intent from substrings or values. Non-client and human-mode errors retain Clap's existing stderr/help behavior.

The envelope path must avoid normal startup. It must not initialize logging, load orchestration configuration, derive/acquire the repository lock, bind a listener, or launch lifecycle work.

## One wait deadline

Construct one `tokio::time::Instant` deadline when `wait` begins. Every potentially blocking step receives or is wrapped by the remaining duration:

- coherent owner observation and each UDS request,
- event/poll recovery,
- repository completion classification,
- local Git subprocesses,
- `git ls-remote` publication checks.

Expiry is an operation outcome, not an incidental transport error. It maps to `timeout` even when the inner operation would otherwise report owner-unavailable or repository-evidence failure after the deadline.

Subprocess helpers must own the child process and kill/reap it when the deadline expires. Correctness tests should synchronize on a fixture accepting a socket connection or a fake Git process starting, then advance/expire the deadline. Wall-clock time is only a generous hang safeguard.

## Header validation

The auth token is opaque secret data but must also be a valid HTTP header value. Reject bytes that HTTP forbids before request construction, especially `\r`, `\n`, other C0 controls, and DEL. The error reports only the environment-variable name and typed reason, never the value. Valid visible/allowed bytes remain unchanged.

## Partial-intent audit

`commands_submitted` is a per-invocation audit list. Build it from successful submission attempts and pass it unchanged into every partial-intent result. A mark found in the initial observation is state, not a command submitted by this invocation.

## Verification

Use the existing `tests/client_cli_tests.rs` fixture stack. Add focused cases rather than another integration framework. Tests must assert stdout object cardinality, exit status, no request bytes for invalid tokens, deterministic deadline cancellation, zero mutation for wait, and exact submitted command names.
