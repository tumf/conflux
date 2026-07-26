# Design: External Lifecycle Integrations

## Goals

- Let normal cflx invocations expose lifecycle state to tools such as Herdr.
- Keep Conflux independent of any specific terminal manager.
- Preserve workspace-local workflow authority and fail-open observability.

## Contract

Configuration supplies one argv array. cflx spawns that command once per process with piped stdin and inherited environment. It writes one compact JSON object per line.

Each message includes:

- protocol version
- monotonically increasing sequence number
- event kind
- semantic state when applicable
- execution mode (`tui` or `run`)
- process ID
- optional non-secret context such as workspace path and change ID

Initial event kinds are `process_started`, `state_changed`, `session_identified`, and `process_stopping`. State values are `idle`, `working`, and `blocked`.

## Runtime Boundary

A lifecycle dispatcher owns the child process, serialization, deduplication, bounded queue, and shutdown deadline. Producers publish typed lifecycle events without knowing the adapter command or transport.

TUI state transitions must be emitted from the TUI state/action layer, not inferred by scraping rendered text. Non-interactive orchestration maps existing typed execution events into the same semantic lifecycle model.

## Failure Policy

The integration is observability-only:

- spawn failure produces a warning and disables the adapter for the process
- broken stdin or early child exit produces a warning and disables further sends
- a full queue drops redundant state refreshes rather than blocking workflow execution
- shutdown waits only for a documented bounded deadline, then terminates the adapter child
- adapter output and exit status never alter apply, acceptance, archive, merge, or resume routing

## Herdr Compatibility

A separate adapter can inspect inherited `HERDR_ENV`, `HERDR_SOCKET_PATH`, and `HERDR_PANE_ID`. When active in a Herdr pane, it translates lifecycle messages to Herdr's socket protocol. Outside Herdr it exits successfully without side effects.

Herdr still needs to recognize the foreground `cflx` process to create and remove the Agent entry. This proposal supplies authoritative lifecycle state; it does not modify Herdr.

## Security and Privacy

Payloads exclude configuration values, prompts, terminal buffers, environment values, tokens, and error bodies by default. Future payload expansion requires explicit specification.

## Alternatives Rejected

- PATH shim: modifies normal command resolution and is outside the target integration model.
- Plugin-owned pane only: does not cover a normal `cflx` invocation.
- Workflow hooks: do not cover TUI process lifecycle or all user-blocking states.
- Dynamic libraries: unnecessary ABI and security complexity.
