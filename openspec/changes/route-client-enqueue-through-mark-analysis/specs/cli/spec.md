## MODIFIED Requirements

### Requirement: Intent-based enqueue

`cflx client enqueue <change-id>` MUST express the high-level intent to admit one change to the existing command-capable owner. The CLI MUST determine the route from authoritative capabilities, instance, state, execution status, and action eligibility. It MUST shield callers from raw command types, `expected_revision`, execution marks, queue intent, and idempotency keys.

For an eligible ordinary target, enqueue MUST use the same target-scoped execution-mark write and shared mark-settlement/analyze admission path as the TUI. It MUST preserve unrelated marks and MUST NOT submit direct queue intent, invoke `DynamicQueue`, or implement a second analyze path. Execution marks and queue intent remain distinct: the mark is human-visible target selection, while queue intent is produced only by the existing analyze/admission services. The CLI MUST return success only after authoritative state proves the target already admitted or admitted through that shared path; a settled mark alone is not admission.

The CLI MUST submit the smallest supported sequence through the existing shared operator-command service, wait for each submitted command record to settle, and then observe bounded authoritative admission evidence. It MUST recompute intent after bounded stale-revision conflicts, use a fresh idempotency identity when the typed command identity changes, and fail if the owner instance changes. It MUST fail closed for unknown, final, blocked, worktree-ineligible, active-run-limited, unsupported, or command-incapable targets without starting another owner or claiming admission. An idle-owner Start MUST NOT consume unrelated execution marks: the client MUST preserve them and return a typed operator-intent conflict if it cannot isolate the requested target through existing semantics. If the requested mark settles but admission does not, the client MUST return non-zero `partial_intent`, identify the remaining mark, and MUST NOT claim rollback or admission. Every `partial_intent` result MUST list every command actually submitted by that invocation, regardless of whether that command settles successfully, and MUST omit commands that were skipped or rejected before submission. Stale-revision recomputation MUST preserve that exact per-invocation audit sequence without duplicates or omissions.

#### Scenario: Idle owner admits one change

**Given**: a command-capable idle owner exposes eligible unmarked change `alpha`
**When**: an agent runs `cflx client enqueue alpha --json`
**Then**: the client marks only `alpha`, rereads authoritative state, and submits Start through the existing command service
**And**: it reports success only after the command records settle successfully

#### Scenario: Live owner admits additional eligible work through analysis

**Given**: a command-capable owner has a live scheduler and `alpha` is eligible for dynamic admission
**When**: an agent runs `cflx client enqueue alpha --json`
**Then**: the client marks only `alpha` through the same shared mark-settlement notification used by TUI mark input
**And**: it preserves unrelated execution marks
**And**: it does not submit `set_queue_intent`, invoke DynamicQueue, start a second scheduler, or implement a second analyze path
**And**: it reports success only after the existing analyze/admission path authoritatively admits `alpha`

#### Scenario: Settled live-owner mark alone is not admission

**Given**: a live owner accepts the requested execution mark but analyze does not admit the target before the bounded client deadline or owner transition
**When**: enqueue evaluates its result
**Then**: it returns non-zero `partial_intent` with the remaining mark and exact submitted-command audit
**And**: it does not claim the target is queued, active, or admitted
**And**: it does not roll back the accepted mark

#### Scenario: Idle enqueue preserves unrelated marks

**Given**: an idle owner has unrelated ordinary change `beta` execution-marked
**When**: an agent runs `cflx client enqueue alpha --json`
**Then**: the client does not submit Start for the combined marked set
**And**: it does not clear or otherwise mutate `beta`'s mark
**And**: it returns `operator_intent_conflict` with a non-zero exit status

#### Scenario: Settled mark without Start is partial intent

**Given**: idle admission settles the requested execution mark
**When**: Start is rejected or a conflicting mark appears before Start
**Then**: enqueue exits non-zero with outcome `partial_intent`
**And**: it identifies the remaining requested mark and warns that a later operator Start can consume it
**And**: it does not claim rollback

#### Scenario: Stale revision is recomputed safely

**Given**: owner state advances between the client's read and mutation
**When**: the v2 command rejects the observed revision as stale
**Then**: the client rereads instance and authoritative state and recomputes the complete intent
**And**: retries are bounded
**And**: no settled side effect is submitted twice

#### Scenario: Headless run is not command capable

**Given**: the socket belongs to `cflx run`, whose remote command executor is unbound
**When**: an agent runs `cflx client enqueue alpha --json`
**Then**: the client returns `owner_not_command_capable` non-zero
**And**: it does not start or replace an owner

#### Scenario: Unsafe target is mutation free

**Given**: `alpha` is unknown, final, blocked, worktree-ineligible, or blocked by active-run iteration-limit evidence
**When**: an agent runs `cflx client enqueue alpha --json`
**Then**: the client returns a typed unsuccessful outcome
**And**: it submits no hidden fallback, manual archive, merge, repair, or second-owner action

#### Scenario: Pre-existing mark is not reported as submitted

**Given**: `alpha` was already execution-marked before the client invocation
**When**: the client skips mark submission, submits Start, and Start fails
**Then**: `partial_intent.detail.commands_submitted` contains `start`
**And**: it does not contain `set_execution_mark`
**And**: the remaining mark and its later-consumption warning are still reported truthfully

#### Scenario: Failed submitted Start remains in the audit list

**Given**: the client submits `Start` and the owner settles it unsuccessfully
**When**: enqueue returns `partial_intent`
**Then**: `detail.commands_submitted` contains `start`
**And**: its order matches the command records actually submitted by this invocation

#### Scenario: Stale revision retry preserves exact command audit

**Given**: one enqueue invocation has a mutation attempt rejected with `StaleRevision` before a command record is created
**When**: the client rereads authoritative state, recomputes intent, and submits the next supported command sequence
**Then**: `detail.commands_submitted` equals the ordered command records actually created by that invocation
**And**: the rejected pre-record attempt is not listed
**And**: no retried command is duplicated or omitted
