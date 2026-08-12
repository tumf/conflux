## MODIFIED Requirements

### Requirement: Stable client output contract

Client commands MUST support concise human output and a machine-readable JSON mode. Every invocation that selects the `client` namespace and includes the exact `--json` flag MUST emit exactly one versioned result envelope on stdout, including CLI parsing and usage failures. Successful and unsuccessful outcomes MUST use stable machine-readable outcome names and exit status; parse failures MUST use outcome `usage_error`. Diagnostics MUST use stderr and MUST NOT contaminate JSON stdout. Non-JSON and non-client Clap errors MUST retain their normal human-facing behavior. Secrets MUST NOT be accepted in argv or emitted in either stream.

#### Scenario: JSON success is one parseable object

**Given**: a compatible owner is available
**When**: an agent runs `cflx client status --json`
**Then**: stdout contains exactly one parseable versioned JSON object
**And**: the command exits zero
**And**: progress or diagnostics do not appear on stdout

#### Scenario: Typed failure remains machine readable

**Given**: no owner is listening at the selected socket
**When**: an agent runs `cflx client status --json`
**Then**: stdout contains one failure envelope with outcome `owner_not_running`
**And**: the command exits non-zero
**And**: no owner is started

#### Scenario: JSON usage failure remains machine readable

**Given**: argv selects `cflx client`, contains the exact `--json` flag, and has an invalid change ID, timeout, missing argument, or unknown option
**When**: Clap rejects the invocation
**Then**: stdout contains exactly one versioned failure envelope with outcome `usage_error`
**And**: the command exits non-zero
**And**: no logging, lock, listener, owner, or repository mutation is initialized

#### Scenario: Human parse behavior remains compatible

**Given**: a client invocation does not request JSON, or argv does not select the client namespace
**When**: Clap rejects the invocation
**Then**: normal human-facing Clap diagnostics and exit behavior remain in effect
**And**: no JSON envelope is emitted merely because another argument value contains the substring `--json`

### Requirement: Intent-based enqueue

`cflx client enqueue <change-id>` MUST express the high-level intent to admit one change to the existing command-capable owner. The CLI MUST determine the route from authoritative capabilities, instance, state, execution status, and action eligibility. It MUST shield callers from raw command types, `expected_revision`, execution marks, queue intent, and idempotency keys.

The CLI MUST submit the smallest supported sequence through the existing shared operator-command service, wait for each command record to settle, and return success only when the target is already admitted or the intended admission is accepted. It MUST recompute intent after bounded stale-revision conflicts, use a fresh idempotency identity when the typed command identity changes, and fail if the owner instance changes. It MUST fail closed for unknown, final, blocked, worktree-ineligible, active-run-limited, unsupported, or command-incapable targets without starting another owner or claiming admission. An idle-owner Start MUST NOT consume unrelated execution marks: the client MUST preserve them and return a typed operator-intent conflict if it cannot isolate the requested target through existing semantics. If the requested mark settles but Start does not, the client MUST return non-zero `partial_intent`, identify the remaining mark, warn that a later operator Start can consume it, and MUST NOT claim rollback. Every `partial_intent` result MUST list only commands actually submitted by that invocation.

#### Scenario: Pre-existing mark is not reported as submitted

**Given**: `alpha` was already execution-marked before the client invocation
**When**: the client skips mark submission and Start then fails
**Then**: `partial_intent.detail.commands_submitted` does not contain `set_execution_mark`
**And**: the remaining mark and its later-consumption warning are still reported truthfully

### Requirement: Observation-only completion wait

`cflx client wait <change-id>` MUST observe one owner and repository until the requested change reaches a repository-verifiable terminal success, a typed unsuccessful terminal outcome, owner replacement, or timeout. One monotonic operation deadline MUST bound initial observation, repeated observation, event/poll recovery, repository classification, and every local or remote Git subprocess. Deadline expiry MUST terminate and reap owned subprocesses, return typed `timeout`, and MUST NOT be replaced by a later inner transport or evidence error. It MUST use event streaming when available and authoritative multi-resource polling to recover from gaps. Reads MUST agree on `instance_id`; revision-bearing resources must reconcile at one `state_revision`, and `event_sequence` must not move backwards. `status` may end bounded rereads with typed observation conflict, while `wait` must keep reconciling until its deadline. API presentation and command records MAY provide progress but MUST NOT alone certify implementation or integration completion.

Wait MUST submit no mutation command. Change disappearance alone MUST NOT count as success. For terminal mode `merged`, success requires the existing repository completion oracle to return `Completed` for the captured base branch. For `base_published`, the selected remote base ref must additionally equal the locally verified base tip. For `branch_pushed`, archived proposal evidence must exist on the named local change branch and the selected remote branch ref must equal that local branch tip; this proves publication, not base integration. `NotCompleted`, `Contradictory`, `EvidenceError`, unsupported mode, and missing or ambiguous repository evidence MUST remain typed non-success outcomes.

#### Scenario: Stalled owner read respects the operation deadline

**Given**: the owner accepts a UDS connection but does not complete a response
**When**: the configured wait deadline expires
**Then**: wait exits non-zero with outcome `timeout`
**And**: it does not wait for the transport's longer per-request timeout
**And**: it submits no mutation

#### Scenario: Stalled remote verification respects the operation deadline

**Given**: terminal-mode verification starts a remote Git lookup that does not complete
**When**: the configured wait deadline expires
**Then**: the Git child is terminated and reaped
**And**: wait exits non-zero with outcome `timeout`
**And**: no later repository evidence error replaces that outcome
