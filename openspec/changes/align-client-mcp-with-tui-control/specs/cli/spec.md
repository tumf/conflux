## MODIFIED Requirements

### Requirement: Existing-owner client namespace

The CLI MUST provide `cflx client` as a client-only namespace for operating one existing repository owner. It MUST provide `status`, explicit execution-mark controls, explicit run lifecycle controls, `wait`, explicit proposal subscription controls, and `mcp`. CLI controls MUST use the same shared operator intents and subscription services as the TUI and MCP. Invoking a client command MUST NOT acquire the orchestration repository lock, bind an owner listener, initialize an orchestration run, launch lifecycle adapters or AI subprocesses, or otherwise become an owner. `cflx run` MUST retain its existing explicit-target owner semantics.

The namespace MUST derive the default Unix socket from the canonical Git common directory and MAY accept an explicit socket override. Authentication secrets MUST be read from a named environment variable rather than a literal argv value. Builds without the required local API support MUST reject the namespace before side effects.

#### Scenario: Client does not compete with the owner

- **GIVEN** a TUI process owns the repository and serves its default Unix socket
- **WHEN** another process runs a client status, mark, lifecycle-control, subscription, or MCP command
- **THEN** it connects as a client to the existing owner
- **AND** it does not acquire the repository lock or start another orchestration process

#### Scenario: Feature-disabled client fails before mutation

- **GIVEN** the binary lacks local remote-control support
- **WHEN** an operator invokes any `cflx client` command
- **THEN** the command exits non-zero with an actionable error
- **AND** it creates no repository lock, API socket, log, or workspace mutation

## REMOVED Requirements

### Requirement: Intent-based enqueue

**Reason**: Admission-oriented client enqueue diverges from TUI control semantics and can bypass execution-mark settlement by writing queue intent directly.

**Migration**: Use explicit mark/unmark followed by explicit start when desired. Marking alone never claims admission.

### Requirement: Existing-owner client MCP namespace

**Reason**: The six historical MCP tools expose admission and execution-sink implementation details rather than the compact TUI-equivalent control and proposal-subscription boundary.

**Migration**: Use `cflx_status`, `cflx_control`, and `cflx_subscribe`.

## ADDED Requirements

### Requirement: Target-scoped client execution-mark control

The client MUST provide explicit mark and unmark operations over a bounded non-empty set of proposal IDs. Each target MUST be written through the existing target-scoped `SetExecutionMark` service used by TUI mark input. The operation MUST preserve every unrelated mark and MUST NOT construct queue intent, Start, Retry, DynamicQueue mutation, analysis, admission polling, or an execution identity. Desired state already satisfied MUST be successful and idempotent.

A multi-target request MUST validate the complete target set before mutation and MUST report per-target settlement coherently. It MUST NOT replace the complete mark store from a caller-supplied list.

#### Scenario: Multiple proposals are marked without replacing existing marks

- **GIVEN** proposal `beta` is marked and proposals `alpha` and `gamma` are unmarked
- **WHEN** the client marks `alpha` and `gamma`
- **THEN** all three proposals are marked
- **AND** no command mutates `beta`
- **AND** no queue or lifecycle command is submitted

#### Scenario: Mark settlement does not claim admission

- **GIVEN** a mark command settles successfully
- **WHEN** the client returns its result
- **THEN** it reports only the requested mark state and whether it changed
- **AND** it returns no admission outcome or execution ID
- **AND** it does not wait for queue or active state

#### Scenario: Unmark is target scoped

- **GIVEN** proposals `alpha` and `beta` are marked
- **WHEN** the client unmarks only `alpha`
- **THEN** `alpha` is unmarked and `beta` remains marked
- **AND** admitted or active work is not stopped or dequeued by the mark write

### Requirement: Client lifecycle control mirrors TUI app controls

The client MUST provide explicit Start, graceful Stop, and ForceStop operations. Each operation MUST submit only the corresponding shared operator intent used by the TUI. Start MUST consume the authoritative current mark set and MUST NOT accept a caller-supplied replacement set. The client MUST NOT reimplement mode, eligibility, retry, analysis, cancellation, scheduler, or stop-classification policy.

#### Scenario: Start is F5 equivalent

- **GIVEN** the owner has an authoritative execution-mark set
- **WHEN** the client explicitly requests Start
- **THEN** the shared Start transaction consumes that mark set exactly as TUI F5 or `!`
- **AND** the client does not create queue intent independently

#### Scenario: Graceful stop uses the shared stop transaction

- **GIVEN** the current mode permits graceful stop
- **WHEN** the client explicitly requests Stop
- **THEN** it submits the same shared Stop intent as the TUI
- **AND** it does not infer process termination before settlement

#### Scenario: Force stop uses shared runtime classification

- **GIVEN** the operator explicitly requests ForceStop
- **WHEN** the client submits the action
- **THEN** the shared ForceStop transaction collects and applies the same runtime activity classification as the TUI
- **AND** the client does not classify or terminate work independently

### Requirement: Compact existing-owner MCP control namespace

`cflx client mcp` MUST expose exactly three tools: `cflx_status`, `cflx_control`, and `cflx_subscribe`. All tools MUST retain the existing immutable call-scoped `project_dir` / `unix_socket` route precedence, authentication, MCP initialization, JSON-RPC validation, bounded frame, protocol-only stdout, and client-only owner constraints.

`cflx_control` MUST accept action `mark`, `unmark`, `start`, `stop`, or `force_stop`. Mark/unmark MUST accept a bounded proposal-ID set; lifecycle actions MUST reject proposal IDs and operate on authoritative owner state. `cflx_subscribe` MUST accept action `set`, `get`, or `clear` over a bounded non-empty proposal-ID set. No tool may expose raw command construction, expected revision, idempotency keys, queue intent, shell interpretation, or hidden admission.

#### Scenario: MCP lists only the compact tools

- **WHEN** an initialized MCP peer requests `tools/list`
- **THEN** it sees exactly `cflx_status`, `cflx_control`, and `cflx_subscribe`
- **AND** it does not see `cflx_enqueue` or historical notify tools

#### Scenario: MCP mark mirrors TUI mark input

- **GIVEN** unrelated proposal `beta` is marked
- **WHEN** MCP control marks proposal `alpha`
- **THEN** the shared mark store contains both `alpha` and `beta`
- **AND** the tool returns without admission polling or execution identity

#### Scenario: MCP lifecycle action is explicit

- **WHEN** MCP control receives action `start`, `stop`, or `force_stop`
- **THEN** it submits only the corresponding shared lifecycle intent
- **AND** mark actions cannot fall through to lifecycle actions

#### Scenario: Two projects share one MCP process safely

- **GIVEN** projects A and B have independent owners
- **WHEN** consecutive tool calls select A and B by absolute project directory
- **THEN** each call contacts only its selected owner
- **AND** no process-global route is mutated

#### Scenario: Invalid route fails before owner contact

- **GIVEN** one call supplies conflicting selectors or an unusable project directory
- **WHEN** route validation runs
- **THEN** it returns the normal bounded MCP validation error
- **AND** no owner is contacted and no repository is mutated

#### Scenario: Tool calls require initialization

- **GIVEN** an MCP peer has not completed initialization
- **WHEN** it requests `tools/list` or `tools/call`
- **THEN** the adapter returns a machine-readable protocol error
- **AND** no owner request or workflow mutation occurs

#### Scenario: MCP stdout remains protocol only

- **WHEN** a tool succeeds or fails
- **THEN** stdout contains only valid MCP JSON-RPC frames
- **AND** diagnostics remain outside the protocol stream
