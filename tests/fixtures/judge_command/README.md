# Judge command compatibility fixtures

Checked-in request/response/error fixtures for the optional purpose-scoped
`judge_commands.parallel_dependency` boundary.

The compatibility baseline is
[`tumf/jev-cli v0.4.1`](https://github.com/tumf/jev-cli/releases/tag/v0.4.1),
resolved to tag commit `50488f35d705051b5d621d1d10e69d1503783e7d`. That
immutable tag — not repository `main` and not live API output — defines:

- batched invocation `jev run -`, which passes complete request JSON through
  unchanged and inserts a default model only when one is omitted;
- one JSON response object on stdout, structured errors on stderr;
- exit `2` for CLI/input errors, `3` for credential/authentication errors, `4`
  for transient API/network errors, and `1` for other API/response failures;
- credential ownership by the command (`TYPESAFE_API_KEY` or its own private
  credential store). Conflux passes no provider key.

Later changes on `jev-cli` repository `main` do not silently change Conflux's
accepted command contract: regenerating these fixtures from a newer baseline is
a reviewed change to this directory.

| File | Role |
|---|---|
| `request_baseline.json` | Complete request object Conflux writes to stdin |
| `response_valid_compact.json` | Accepted compact response |
| `response_valid_pretty.json` | Accepted pretty-printed response |
| `error_stderr_invalid_request.json` | Structured stderr payload accompanying exit `2`; never an answer source |

Consumed by `src/judge_command.rs` unit tests via `include_str!`.
