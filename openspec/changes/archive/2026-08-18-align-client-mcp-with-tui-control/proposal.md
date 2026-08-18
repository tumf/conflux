---
change_type: implementation
priority: high
dependencies: []
references:
  - openspec/CONSTITUTION.md
  - openspec/specs/cli/spec.md
  - openspec/specs/tui-architecture/spec.md
  - openspec/specs/operator-command-execution/spec.md
  - openspec/specs/remote-control-api/spec.md
  - openspec/specs/external-lifecycle-integrations/spec.md
  - openspec/specs/documentation/spec.md
  - src/client/mcp.rs
  - src/client/enqueue.rs
  - src/orchestration/operator_command.rs
  - src/orchestration/mark_settlement.rs
  - src/web/completion_sink.rs
verifications:
  - id: client-tui-control-parity
    requirement: "MCP mark, start, and stop controls use only the same shared operator intents as TUI controls; mark writes preserve unrelated marks and cannot directly mutate queue intent"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused Rust unit and integration output for exact command sequences, target-scoped mark preservation, start/stop parity, stale revisions, owner replacement, and absence of client queue mutation"
    rerun: "cargo test --lib client:: && cargo test --test client_cli_tests control"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
  - id: proposal-subscription-tests
    requirement: "Explicit proposal-scoped subscriptions support multiple proposal IDs, bind future execution episodes at owner admission, and deliver notification without auto-resuming an agent"
    phase: pre-integration
    owner: conflux-acceptance
    trigger: pull-request-validation
    automation: Makefile
    evidence: "Focused registry, MCP, delivery, replay, late-registration, owner-restart, and auto-resume-removal tests"
    rerun: "cargo test --lib completion_sink && cargo test --test client_mcp_integration subscribe"
    prerequisites: []
    execution_class: repository-local
    completion_role: change-blocking
---

# Align client MCP with TUI control and explicit proposal subscriptions

**Change Type**: implementation

## Premise / Context

- Execution marks are operator-selected execution targets. Queue intent is separate scheduler-owned admission state.
- TUI mark/unmark changes only the process-local mark set. Existing mark settlement and owner-side analysis may later admit stable marked work.
- TUI F5/`!`, graceful stop, and force stop already delegate to shared run-control intents.
- Current `cflx_enqueue` does not mirror this boundary. For a live owner it submits `SetQueueIntent(true)` directly and can dispatch a proposal while `execution_marked` remains false.
- Current Hermes/OpenCode auto-resume integrations infer callback registration from an admitted enqueue envelope. The user has rejected automatic resume and wants explicit, easy callback control instead.
- Existing execution-scoped completion sinks provide safe bounded argv delivery, but require an execution ID that does not exist when an operator marks work before admission.

## Problem / Context

The MCP surface currently conflates three separate responsibilities:

1. operator selection through execution marks;
2. lifecycle control through start/stop;
3. execution notification registration.

`cflx_enqueue` chooses admission policy, can skip mark settlement, and returns an execution binding. Auto-resume hooks then register callbacks implicitly. This differs from TUI behavior and makes the client an orchestration path rather than a thin control adapter.

The surface is also fragmented: status, enqueue, wait, and three notify tools expose implementation history rather than the small set of actions an agent needs.

## Proposed Solution

Expose three MCP tools:

- `cflx_status`
- `cflx_control`
- `cflx_subscribe`

`cflx_wait` is removed only from MCP. `cflx client wait` remains the bounded CLI completion oracle. MCP hosts that need asynchronous completion explicitly register a callback with `cflx_subscribe`; an MCP host that cannot execute callback argv has no MCP completion oracle, by design.

### `cflx_status`

Retain coherent read-only owner, proposal, mark, queue, mode, and execution observation.

### `cflx_control`

Accept one action and an optional bounded set of one through 64 distinct change IDs (called proposals in user-facing text):

- `mark`: set the named proposals' execution marks to true;
- `unmark`: set the named proposals' execution marks to false;
- `start`: invoke the same shared Start intent as TUI F5/`!`, consuming the authoritative current mark set rather than a caller-supplied replacement set;
- `stop`: invoke the same shared graceful Stop intent as TUI stop control;
- `force_stop`: invoke the same shared ForceStop intent as TUI immediate stop.

`mark` and `unmark` are target-scoped desired-state writes. They preserve unrelated marks and return after command settlement without waiting for admission. The MCP adapter MUST NOT construct `SetQueueIntent`, invoke DynamicQueue, run analysis, poll admission, synthesize execution IDs, or start a second owner.

`start`, `stop`, and `force_stop` delegate to the existing shared operator transaction. The adapter does not reimplement mode, eligibility, retry, analyze, cancellation, or scheduler policy.

### `cflx_subscribe`

Accept one subscription action over one through 64 distinct change IDs:

- `set`: atomically register or replace one proposal-scoped subscription for every named proposal;
- `get`: inspect subscriptions for the named proposals;
- `clear`: atomically clear subscriptions for the named proposals.

A subscription is process-local observability state, not workflow state. It can be registered before admission. When a subscribed proposal enters a new execution episode, the owner binds that episode to the proposal subscription and delivers terminal `completed`, `failed`, or `stopped` once; optional `blocked` remains edge-triggered. The callback event contains the actual `instance_id`, `execution_id`, and wire-compatible `change_id`; `CFLX_CHANGE_ID` remains the environment name.

A proposal subscription applies to the current live episode and future execution episodes until cleared or the owner exits. Each episode has independent delivery dedupe. Re-admission with a new execution ID produces a distinct notification. Replace changes callback configuration for undelivered current/future events; clear cancels pending delivery but does not terminate an already-started callback. Set, replace, clear, then set MUST NOT redeliver a terminal event already delivered for the same episode. Registration after the latest episode is terminal immediately attempts delivery only when that terminal edge has not already been delivered by this owner. Owner restart invalidates subscriptions and retained episode history.

`set` takes bounded argv executed directly without shell interpretation and is Unix-socket-only. Existing completion-sink safety rules for command length, environment scrubbing, artifact ownership, bounded execution, failure isolation, and secret-free diagnostics remain mandatory.

Remove automatic registration and automatic session/agent resume:

- delete the OpenCode and Hermes auto-resume plugins, post-tool hooks, examples, tests, and canonical requirements;
- retain explicit callback delivery as observability only;
- no callback invokes or resumes an agent loop automatically;
- agents explicitly call `cflx_subscribe` when they want notification.

The CLI counterpart uses the same shared implementations. It MAY preserve separate human verbs (`mark`, `unmark`, `start`, `stop`, `force-stop`, `subscribe`) while the MCP groups them into `control` and `subscribe`; it MUST NOT retain admission-oriented `enqueue` semantics or an alias that hides mark-only behavior.

## Acceptance Criteria

1. MCP lists exactly `cflx_status`, `cflx_control`, and `cflx_subscribe`.
2. `cflx_control mark/unmark` accepts 1–64 distinct change IDs, uses target-scoped `SetExecutionMark`, preserves unrelated marks, and returns without observing admission.
3. No MCP/client control path constructs `SetQueueIntent`, invokes DynamicQueue, implements analysis, polls admission, or synthesizes an execution ID from a mark.
4. `cflx_control start/stop/force_stop` invokes the same shared operator intents and mode matrix as TUI F5/stop controls.
5. `cflx_subscribe set/get/clear` accepts 1–64 distinct change IDs; set/clear are atomic and get is bounded and named-target only.
6. A subscription can precede admission, follows each new execution episode of that proposal until cleared, and emits the actual execution binding in each event.
7. Callback registration and delivery do not create workflow command records, advance state revision, change marks/queue/mode, or alter workflow outcome.
8. OpenCode/Hermes auto-resume hooks and examples are removed. No tool result triggers automatic sink registration or agent/session resume.
9. Existing callback sandboxing, boundedness, dedupe, late-terminal delivery, owner-incarnation, and failure-isolation guarantees remain.
10. Canonical MODIFIED requirements retain all unrelated existing scenarios; removed auto-resume requirements are explicit REMOVED deltas.
11. `cflx_wait` and its completion oracle remain available through CLI but are absent from MCP.
12. Existing MCP route resolution, JSON-RPC initialization/error behavior, protocol-only stdout, and bounded frame guarantees remain unchanged.

## Explicit Completion Conditions

- `src/client/enqueue.rs`, admission-oriented `Operation::Enqueue`, and all `cflx_enqueue` MCP/CLI dispatch code are deleted; unreachable dead-code retention does not satisfy completion.
- Client-side source contains no construction of `SetQueueIntent`, `Start` as a consequence of mark fall-through, or `RetryChange` as a consequence of mark input.
- `cflx_control start` constructs Start only for explicit action `start`; mark/unmark never falls through to lifecycle control.
- `cflx_subscribe` has bounded multi-proposal validation and all-or-nothing set/clear behavior.
- Proposal subscriptions bind at owner admission and deliver independent events for successive execution IDs.
- `examples/integrations/opencode-auto-resume/`, `examples/integrations/hermes-auto-resume/`, `tests/opencode_auto_resume_example.rs`, and `tests/hermes_auto_resume_example.rs` are removed. Stale links/assertions are removed from `README.md`, `AGENTS.md`, `tests/client_cli_tests.rs`, `skills/cflx-run/SKILL.md`, and `skills/cflx-run/references/cflx-run.md`; embedded skill output contains no auto-resume or enqueue guidance.
- OpenAPI/MCP schemas, help, README, bundled skill docs, AGENTS guidance, and canonical specs describe explicit control and subscription only.
- `client-tui-control-parity` and `proposal-subscription-tests` pass.
- `cflx openspec validate align-client-mcp-with-tui-control --archive-gate` passes.

## Scope Rationale

Mark/control parity, MCP contraction, proposal-scoped subscriptions, and auto-resume removal must land together. Keeping admission-oriented enqueue or implicit hooks during a staged migration would retain the analyze bypass or silently register callbacks from a mark-only result.

## Retired Scenarios

Scenarios this change deliberately retires from a MODIFIED requirement, rather
than losing by accident. Declared here because the promotion-safety regression
treats every other disappearance as a coverage regression, and because a
declaration inside a delta block would be copied verbatim into a canonical spec
that should describe the system rather than the history of one change.

- remote-control-api: Client observation does not alter API semantics / Enqueue uses ordinary typed commands
  — replaced by `Client controls use ordinary typed commands`, which asserts
  strictly more: every mutation is still an ordinary v2 command record taking the
  same shared operator intent as the equivalent TUI control, *and* mark/unmark
  never submits queue intent or a lifecycle command.
- cli: Existing-owner client MCP namespace / MCP enqueues into the existing TUI
  — the tool it describes no longer exists. `MCP lists only the compact tools`
  asserts the surface, and `Raw workflow commands are not exposed` keeps the
  "does not become a second owner" half.
- cli: MCP tool calls remain bounded / Long-lived TUI does not hold enqueue open
  — replaced by `Long-lived TUI does not hold a control call open`, which states
  the same bound for the tool that replaced it.
- cli: Direct client completion notification management / Operator registers one callback for an explicit project
  — replaced by `Operator registers callbacks for explicit proposals`, which
  covers the same routing and argv rules over one *or more* proposals.
- cli: Direct client completion notification management / Operator inspects and clears one callback
  — replaced by `Operator inspects and clears named subscriptions`, which adds
  that clearing removes only the named targets.
- cli: Direct client completion notification management / Installed operation skill teaches the direct CLI path
  — replaced by `Installed operation skill teaches explicit subscription`, which
  additionally requires the skill to state that notification resumes no agent.

`Wait certifies evidence from the selected project` is *not* retired: it moves
from the MCP requirement, whose wait tool is withdrawn, to the CLI namespace
requirement that still owns the completion oracle.

## Out of Scope

- Merging execution marks with queue intent.
- Changing the 10-second mark-stability window or owner-side mark analysis.
- Changing TUI keybindings or shared Start/Stop policy.
- Durable subscriptions across owner restart.
- Shell callback interpretation, remote TCP callback mutation, or callback success affecting workflow outcome.
- Automatic agent/session resume from callback delivery.

The tracked Rust hooks are path-scoped and will run workspace-wide formatting and Clippy when implementation paths are staged. Requirement-specific focused tests remain explicit implementation evidence.
