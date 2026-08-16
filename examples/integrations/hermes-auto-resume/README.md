# Hermes auto-resume (reference integration)

Bind one admitted Conflux execution to the Hermes messaging thread that asked
for it, and send that thread the typed event when the execution finishes.

This is **reference material**. It lives in the repository, not in the `cflx`
crate: the `include` allowlist in `Cargo.toml` does not name `/examples`, so
`cargo package` cannot ship it and `cflx install-skills` does not install it.
Copy it into `~/.hermes/plugins/`, read it, and change it to fit your setup.

## What it does

1. A Hermes `post_tool_call` hook watches for one tool: `cflx_enqueue` (or a
   segment-exact namespaced `<server>_cflx_enqueue`).
2. On a supported, successful, **admitted** envelope it takes the
   `(instance_id, execution_id, change_id)` binding out of the result, the
   messaging platform / chat / thread out of the *request-scoped* Hermes session
   context, and the Conflux project out of that call's own `project_dir`
   argument (or its low-level `unix_socket`, when that was the call's selector).
3. It runs `cflx client notify set` against the owner **that call named** to
   register one execution-scoped callback argv.
4. When that execution reaches a terminal classification, the Conflux owner runs
   the callback once. The callback rebuilds `HOME`, `PATH` and `HERMES_HOME`,
   and invokes `hermes send --quiet --to <platform:chat[:thread]> <message>`.
5. The Hermes bot-authored Slack message starts with `[AUTO: ...]` and includes
   `event: completed` for a successful terminal event. A responder that treats
   this marker as a continuation signal can then add a concrete follow-up turn
   for the original work. The callback itself does not call or wake an agent.

Nothing here polls, waits, watches a file, or keeps a Hermes
turn alive. There is no API Server call and no webhook. Delivery and automatic
continuation are separate: this integration sends the Hermes bot message, while
the configured responder decides whether and how to continue.

The responder is a deployment prerequisite, not part of this example. It must
observe the Slack bot post itself and turn the `[AUTO: ...]` / `event: ...`
contract into a follow-up Hermes turn. `hermes send` does not write an assistant
message to Hermes session state, and Hermes own Slack ingress ignores its bot
echo, so neither path supplies that observation automatically.

## Why the plugin and not the model

MCP does not tell a server which Hermes turn called it, and a model asked to
construct a callback command is prompt compliance, not engineering. The
`post_tool_call` hook is the one place that sees both halves — the tool result
and the messaging context the call was made in — so the plugin registers the
sink while the return address is still known.

## Install

```bash
mkdir -p ~/.hermes/plugins/cflx-auto-resume
cp examples/integrations/hermes-auto-resume/{__init__.py,cflx_hermes_resume.py,plugin.yaml,README.md} \
   ~/.hermes/plugins/cflx-auto-resume/
hermes plugins list
hermes plugins enable cflx-auto-resume
```

The `plugin.yaml` manifest declares `provides_hooks: [post_tool_call]`, which is
the only hook this plugin registers. It registers no tool and no slash command.

Requirements:

- A resident, command-capable Conflux owner (`cflx` or `cflx tui`) in the target
  repository. A headless `cflx run` binds no command executor and answers
  `owner_not_command_capable`.
- A `hermes` executable and a profile that can already deliver to the target —
  verify that *before* relying on the callback, see below.

## Configuration

Everything is optional; every value is non-secret and ends up in an argv the
owner will execute.

| Variable | Default | Meaning |
| --- | --- | --- |
| `CFLX_BIN` | `cflx` | The Conflux client used to register the sink. |
| `CFLX_UNIX_SOCKET` | unset | Compatibility fallback `--unix-socket`, used **only** for a host that exposes no tool-arguments object at all. Any call-scoped selector always wins. |
| `CFLX_AUTH_TOKEN_ENV` | unset | *Name* of the variable holding the owner token. Never the token. |
| `CFLX_HERMES_BIN` | `which hermes` | Absolute Hermes executable the callback runs. |
| `CFLX_HERMES_HOME` | `$HERMES_HOME`, else `~/.hermes` | `HERMES_HOME` the callback sets. |
| `CFLX_HERMES_CALLBACK_HOME` | `~` | `HOME` the callback sets. |
| `CFLX_HERMES_CALLBACK_PATH` | `$PATH` | `PATH` the callback sets; every entry must be absolute. |
| `CFLX_HERMES_CALLBACK` | next to `__init__.py` | Callback script to register. |

There is no credential among them, and that is the design: `hermes send` reads
platform tokens from the profile `HERMES_HOME` names, so this integration never
holds one, never registers one, and has none to leak.

## Where the reply goes

The destination comes only from the Hermes request the enqueue was made in:

- `HERMES_SESSION_PLATFORM` — the platform, e.g. `slack`
- `HERMES_SESSION_CHAT_ID` — the chat; required, because `--to telegram` alone
  means the profile's *home* channel, which is somebody else's chat
- `HERMES_SESSION_THREAD_ID` — the thread, when the platform has one
- `HERMES_SESSION_SOURCE` — the surface, checked so a CLI/TUI/desktop turn is
  refused rather than addressed

These are read through `gateway.session_context`, which binds them per asyncio
task. The process-global `os.environ` mirror is read **only** when this process
has never bound a session (a bare CLI, cron, or a test): the gateway serves
turns concurrently, and reading the mirror from inside a hook is exactly how a
reply lands in a stranger's chat.

A turn with no messaging platform, no chat ID, or a non-messaging surface
(`api_server`, `cli`, `tui`, `desktop`, `webhook`, `local`, …) registers
**nothing**. The denylist mirrors `NON_MESSAGING_SESSION_SURFACES` in Hermes'
own `gateway/session_context.py` and is default-allow, so a newly added chat
platform works before this list learns its name.

List what your profile can actually reach:

```bash
hermes send --list
hermes send --list slack
```

## Which Conflux owner it registers with

A completion sink is stored by the process that will run it, and an
`execution_id` is process-local to the owner that admitted it. The registration
therefore has to reach the *same* owner the enqueue did — and the only thing
that knows which owner that was is the call itself.

The selector is a **project directory**, not a socket. A socket path names one
owner incarnation's transport and lives under a Git common directory you have to
go and find; a project directory is the stable identity of the work. `cflx
client mcp` accepts `project_dir` on every tool, so register the server once
with no project in it and let each call name its own:

```bash
# The MCP server registration. Leave the connection options off: a server-level
# route is a silent default. A call that omits `project_dir` still reaches it, so
# the enqueue succeeds while the hook sees no route in the call at all — and the
# registration then has nothing authoritative to use.
cflx client mcp
```

```json
{
  "name": "mcp__cflx__cflx_enqueue",
  "arguments": {
    "change_id": "add-my-change",
    "project_dir": "/absolute/path/to/repo"
  }
}
```

`project_dir` must be **absolute**, and it may be any directory inside the
project's Git working tree — the working-tree root, a subdirectory, a linked
worktree, or a submodule. Conflux resolves it the same way the repository lock
does: it derives the canonical repository root, uses that as the repository
evidence `cflx_wait` certifies completion from, and connects to
`<git-common-dir>/cflx-api.sock`. Both halves come from the same selected
project, so no call can pair one project's owner with another's evidence.

The hook reads that exact call-scoped `project_dir` and registers with
`cflx client --project-dir <that> notify set …`. Two calls in one Hermes
process, for two repositories, reach two owners: the route is derived from each
call's own arguments, so there is no project-to-socket map to go stale and no
ordering between concurrent turns that can move either one.

### The low-level `unix_socket` override

`unix_socket` remains available on every tool and on the `cflx client`
namespace as `--unix-socket`, for diagnostics, tests, and owners that are not
reachable through a repository — an owner started with `--web-unix-socket PATH`,
for instance. It is the low-level route: prefer `project_dir`.

The two are **mutually exclusive within one call**. A call supplying both is
refused through the normal MCP validation error (or, on the CLI, the usual
usage error) *before* any owner is contacted; it is not a new envelope outcome,
because no owner conversation happened. A call supplying just `unix_socket` is
registered with `cflx client --unix-socket <that> notify set …`, unchanged.

To find an owner's socket by hand:

```bash
git -C /path/to/repo rev-parse --git-common-dir
```

### Migrating from `CFLX_UNIX_SOCKET`

`CFLX_UNIX_SOCKET` still works, as a **fallback** for a legacy host that exposes
no call arguments to its hooks at all. It is process-global, so it can describe
one project and no more; any call-scoped selector always overrides it. Pass
`project_dir` per call and the variable can go.

Either way, resolution fails closed:

- A call that *names* a route this plugin cannot use — not a string, empty,
  relative, or both selectors at once — registers **nothing**. It is not quietly
  sent to whatever `CFLX_UNIX_SOCKET` happens to hold, because that is exactly
  the cross-project misroute the call-scoped selector exists to prevent.
- A call whose host *did* expose arguments but which named no route registers
  **nothing**. That call used the MCP server's own current-directory or
  namespace default, which this process cannot observe; guessing from the
  environment would register one project's execution against another's owner.
- A host with no arguments object and no usable `CFLX_UNIX_SOCKET` registers
  **nothing** rather than letting the client derive a default from the Hermes
  gateway's working directory, which is not the project's.



## Test delivery before you trust it

The callback runs once, hours later, with nobody watching. Prove the adapter
works first, in the same scrubbed shape the owner will use:

```bash
env -i \
  HOME="$HOME" PATH="/usr/local/bin:/usr/bin:/bin" HERMES_HOME="$HOME/.hermes" \
  "$(command -v hermes)" send --quiet --to slack:C0123ABCD:1786797000.000100 \
  "[AUTO: Conflux execution event — not user-authored] delivery test"
echo "exit $?"
```

`hermes send` exits `0` on delivery, `1` when the platform refused, `2` on
usage. If that fails, the callback will fail the same way.

## Read the registration back

```bash
EXEC=$(cflx client enqueue add-my-change --json | jq -r .execution_id)
cflx client notify get add-my-change "$EXEC" --json
```

`get` returns the registered argv only over the owner's Unix socket — a channel
that may not register a command may not read one back. Over TCP it answers
`sink_registered`, execution state and delivery history with `sink: null`.

## What the callback receives, and what it does not

Conflux **replaces** the callback environment with exactly five variables:
`CFLX_EVENT_PATH`, `CFLX_EVENT_TYPE`, `CFLX_EXECUTION_ID`, `CFLX_CHANGE_ID`,
`CFLX_INSTANCE_ID`. No owner token, no Conflux configuration, no inherited
`PATH`. That is why the registered argv names the interpreter, the callback and
the Hermes executable absolutely, and spells out `HOME`, `PATH` and
`HERMES_HOME` — the callback sets those three and nothing else.

The callback requires all five variables, bounds the event file, checks its
schema version, and refuses an event whose `event_type`, `execution_id`,
`change_id` or `instance_id` disagrees with the environment it was called with.

## Security boundaries

- **argv, never shell.** `cflx client notify set … -- …` stores an argv the
  owner executes directly: no `sh -c`, no quoting, no expansion. The messaging
  target is one argv element, and a chat or thread ID containing `:`, a space or
  a control character is refused rather than reinterpreted.
- **The destination is fixed at registration.** It comes from the Hermes
  request, never from the event file. Nothing Conflux later writes can move it.
- **The event is data.** Fields interpolated into the message are stripped of
  control characters and truncated. The message opens with
  `[AUTO: Conflux execution event — not user-authored]` and tells the receiving agent to
  verify the repository rather than believe the text.
- **No secret ever enters argv.** The only credential in play is the platform
  token in your Hermes profile, read by `hermes send` itself. Diagnostics carry
  a bounded slice of adapter output and no environment dump.
- **`PATH` entries must be absolute.** An empty entry (`:` leading, trailing or
  doubled) means the current directory to every exec that reads it, and the
  callback's working directory is whatever the owner had.
- **Delivery is observability.** A callback that fails, hangs or exits non-zero
  cannot roll back, retry or re-classify anything. Registration success,
  callback exit zero, and message arrival are three separate facts.

## Notification is not success

A delivered message says the owner classified the execution. It does not say the
change succeeded. `completed` is certified from repository evidence, so verify
it the same way before reporting anything:

```bash
cflx client wait add-my-change --timeout 45m --json
cflx client status --json
git log --oneline -5
```

Treat `owner_stopping`, a missing execution, and a replaced owner as non-success
whenever they arrive. Execution IDs and registrations are process-local: a
`kill -9` takes them with it, and no callback runs after abrupt owner death.

## Tests

`tests/hermes_auto_resume_example.rs` drives this example with a fake `cflx` and
a fake `hermes` executable — no live gateway, no owner, no credentials:

```bash
cargo test --features heavy-tests --test hermes_auto_resume_example
```

The suite launches isolated Python subprocesses and fake executables, so the
whole integration-test binary is feature-gated as `heavy-tests` and does not run
in the default unit-test suite.
