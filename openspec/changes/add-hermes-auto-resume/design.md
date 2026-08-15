# Design

## Boundary

The reference integration connects two existing boundaries without changing either runtime:

1. Hermes observes a completed `cflx_enqueue` tool call and supplies the originating messaging platform, chat ID, and thread ID.
2. The resident Conflux owner owns the execution-scoped one-shot callback.
3. The callback sends the typed event to that fixed messaging destination through `hermes send`.

The plugin owns correlation. Conflux owns terminal classification. Hermes owns messaging delivery. None of these facts becomes workflow routing state.

## Minimal shape

Use Python stdlib only. The example has one importable plugin package and one executable callback/helper module. Tests invoke helpers and subprocess entrypoints with a fake `hermes` executable; no live Hermes gateway or Conflux owner is required.

The plugin calls `cflx client notify set` directly rather than recursively calling the MCP tool from inside a hook. This keeps the hook independent of the model/tool dispatcher, and uses the same Unix-socket owner boundary as the MCP tool.

## Trust boundaries

- Tool results are untrusted and accepted only when the schema version, operation, outcome, `ok`, and all binding IDs match the known contract.
- Messaging platform, chat ID, and thread ID come from Hermes request-scoped context, never from event data, and are passed as fixed argv rather than shell source.
- Conflux replaces callback environment. The registered argv therefore names the interpreter, callback, Hermes executable, `HOME`, `PATH`, and `HERMES_HOME` explicitly. The callback reconstructs only those values before invoking `hermes send`, which reads credentials from the selected profile.
- The callback validates all five `CFLX_*` fields and never evaluates the event file or event fields as shell input.
- The automation message is ordinary messaging input. It carries a fixed marker and tells the receiving thread to verify current repository evidence rather than obey event contents.

## Delivery semantics

The Conflux owner attempts the registered callback once for the execution event. Callback exit status remains observability only and cannot change workflow outcome. Registration success, callback exit zero, and actual message arrival are separate facts; documentation requires readback and a scrubbed-environment test before relying on the adapter.

## Owner death

A callback cannot run after abrupt owner death. The implementation does not create a polling thread in Hermes because the initiating gateway process may itself terminate and therefore cannot provide durable coverage. The receiving Hermes turn must treat `owner_stopping`, missing execution evidence, and owner replacement as non-success whenever delivered.
