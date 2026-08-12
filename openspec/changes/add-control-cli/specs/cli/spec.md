## ADDED Requirements

### Requirement: Existing-owner control namespace

The CLI MUST provide `cflx control` as a client-only namespace for operating one existing repository owner. It MUST provide only `status`, `enqueue`, and `wait` initially. Invoking a control command MUST NOT acquire the orchestration repository lock, bind a listener, initialize an orchestration run, launch lifecycle adapters or AI subprocesses, or otherwise become an owner. `cflx run` MUST retain its existing explicit-target owner semantics.

The namespace MUST derive the default Unix socket from the canonical Git common directory and MAY accept an explicit socket override. Authentication secrets MUST be read from a named environment variable rather than a literal argv value. Builds without the required local API support MUST reject the namespace before side effects.

#### Scenario: Control does not compete with the owner

**Given**: a TUI process owns the repository and serves its default Unix socket
**When**: another process runs `cflx control status --json`
**Then**: it connects as a client to the existing owner
**And**: it does not acquire the repository lock or start another orchestration process

#### Scenario: Run retains finite-owner meaning

**Given**: no Conflux process owns the repository
**When**: an operator runs `cflx run alpha`
**Then**: existing explicit-target orchestration behavior remains unchanged
**And**: the invocation is not treated as a control-client request

#### Scenario: Feature-disabled control fails before mutation

**Given**: the binary lacks local remote-control support
**When**: an operator invokes any `cflx control` command
**Then**: the command exits non-zero with an actionable error
**And**: it creates no repository lock, API socket, log, or workspace mutation

### Requirement: Stable control output contract

Control commands MUST support concise human output and a machine-readable JSON mode. JSON mode MUST emit exactly one versioned result envelope on stdout. Successful and unsuccessful outcomes MUST use stable machine-readable outcome names and exit status; diagnostics MUST use stderr and MUST NOT contaminate JSON stdout. Secrets MUST NOT be accepted in argv or emitted in either stream.

#### Scenario: JSON success is one parseable object

**Given**: a compatible owner is available
**When**: an agent runs `cflx control status --json`
**Then**: stdout contains exactly one parseable versioned JSON object
**And**: the command exits zero
**And**: progress or diagnostics do not appear on stdout

#### Scenario: Typed failure remains machine readable

**Given**: no owner is listening at the selected socket
**When**: an agent runs `cflx control status --json`
**Then**: stdout contains one failure envelope with outcome `owner_not_running`
**And**: the command exits non-zero
**And**: no owner is started

### Requirement: Intent-based enqueue

`cflx control enqueue <change-id>` MUST express the high-level intent to admit one change to the existing command-capable owner. The CLI MUST determine the route from authoritative capabilities, instance, state, execution status, and action eligibility. It MUST shield callers from raw command types, `expected_revision`, execution marks, queue intent, and idempotency keys.

The CLI MUST submit the smallest supported sequence through the existing shared operator-command service, wait for each command record to settle, and return success only when the target is already admitted or the intended admission is accepted. It MUST recompute intent after bounded stale-revision conflicts, use a fresh idempotency identity when the typed command identity changes, and fail if the owner instance changes. It MUST fail closed for unknown, final, blocked, worktree-ineligible, active-run-limited, unsupported, or command-incapable targets without starting another owner or claiming admission. An idle-owner Start MUST NOT consume unrelated execution marks: the client MUST preserve them and return a typed operator-intent conflict if it cannot isolate the requested target through existing semantics.

#### Scenario: Idle owner admits one change

**Given**: a command-capable idle owner exposes eligible unmarked change `alpha`
**When**: an agent runs `cflx control enqueue alpha --json`
**Then**: the client marks only `alpha`, rereads authoritative state, and submits Start through the existing command service
**And**: it reports success only after the command records settle successfully

#### Scenario: Live owner admits additional eligible work

**Given**: a command-capable owner has a live scheduler and `alpha` is eligible for dynamic admission
**When**: an agent runs `cflx control enqueue alpha --json`
**Then**: the client uses the existing live-owner admission semantics
**And**: it does not start a second scheduler or owner

#### Scenario: Idle enqueue preserves unrelated marks

**Given**: an idle owner has unrelated ordinary change `beta` execution-marked
**When**: an agent runs `cflx control enqueue alpha --json`
**Then**: the client does not submit Start for the combined marked set
**And**: it does not clear or otherwise mutate `beta`'s mark
**And**: it returns `operator_intent_conflict` with a non-zero exit status

#### Scenario: Stale revision is recomputed safely

**Given**: owner state advances between the client's read and mutation
**When**: the v2 command rejects the observed revision as stale
**Then**: the client rereads instance and authoritative state and recomputes the complete intent
**And**: retries are bounded
**And**: no settled side effect is submitted twice

#### Scenario: Headless run is not command capable

**Given**: the socket belongs to `cflx run`, whose remote command executor is unbound
**When**: an agent runs `cflx control enqueue alpha --json`
**Then**: the client returns `owner_not_command_capable` non-zero
**And**: it does not start or replace an owner

#### Scenario: Unsafe target is mutation free

**Given**: `alpha` is unknown, final, blocked, worktree-ineligible, or blocked by active-run iteration-limit evidence
**When**: an agent runs `cflx control enqueue alpha --json`
**Then**: the client returns a typed unsuccessful outcome
**And**: it submits no hidden fallback, manual archive, merge, repair, or second-owner action

### Requirement: Observation-only completion wait

`cflx control wait <change-id>` MUST observe one owner and repository until the requested change reaches a repository-verifiable terminal success, a typed unsuccessful terminal outcome, owner replacement, or timeout. It MUST use event streaming when available and authoritative multi-resource polling to recover from gaps. Reads MUST agree on `instance_id` and `state_revision`; incompatible reads require bounded reread or typed observation conflict. API presentation and command records MAY provide progress but MUST NOT alone certify implementation or integration completion.

Wait MUST submit no mutation command. Change disappearance alone MUST NOT count as success. For owner terminal mode `merged`, success requires archived proposal evidence and Git ancestry from the owner-published terminal commit to the owner-published base branch. For terminal mode `pushed`, success requires archived proposal evidence plus owner-published selected-remote and remotely confirmed terminal commit evidence. Missing or ambiguous typed owner evidence MUST fail closed rather than weaken truthful completion.

#### Scenario: Wait proves successful local integration

**Given**: `alpha` is processed by one owner in local integration mode
**When**: archive completes and repository evidence proves the resulting change is integrated into the intended base
**Then**: `cflx control wait alpha --json` exits zero with outcome `completed`
**And**: it identifies the observed owner instance and repository completion evidence

#### Scenario: Wait proves pushed publication

**Given**: `alpha` is processed by one owner in pushed terminal mode
**When**: the owner reports terminal `pushed`, an archived proposal exists, and typed owner evidence identifies the selected remote and remotely confirmed terminal commit
**Then**: `cflx control wait alpha --json` exits zero with outcome `completed`
**And**: it does not substitute local branch ancestry for remote confirmation

#### Scenario: Mixed observation is not completion

**Given**: state and execution-contract reads carry different revisions or owner incarnations
**When**: wait evaluates terminal completion
**Then**: it performs a bounded coherent reread or returns `observation_conflict`
**And**: it does not combine the mixed values into a success claim

#### Scenario: Disappearance does not prove success

**Given**: `alpha` disappears from the active snapshot without repository evidence proving archive and integration
**When**: wait evaluates completion
**Then**: it does not return successful completion
**And**: it continues observing or returns a typed unsuccessful outcome

#### Scenario: Wait never repairs execution

**Given**: `alpha` enters an error, blocked, stalled, merge-wait, or retryable state
**When**: wait observes that state
**Then**: it submits no start, retry, queue, resolve, archive, merge, cleanup, or worktree command
**And**: it reports or continues according to the typed terminal/progress contract

#### Scenario: Owner replacement invalidates the wait

**Given**: wait captured one process `instance_id`
**When**: the socket begins serving a different owner incarnation
**Then**: wait exits non-zero with outcome `owner_restarted`
**And**: it does not infer settlement of commands or work owned by the prior process

#### Scenario: Timeout is not completion

**Given**: the configured wait duration expires before verified success or another terminal outcome
**When**: wait reaches the deadline
**Then**: it exits non-zero with outcome `timeout`
**And**: it performs no mutation while exiting
