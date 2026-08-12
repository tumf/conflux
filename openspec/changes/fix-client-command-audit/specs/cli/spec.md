## MODIFIED Requirements

### Requirement: Intent-based enqueue

`cflx client enqueue <change-id>` MUST express the high-level intent to admit one change to the existing command-capable owner. The CLI MUST determine the route from authoritative capabilities, instance, state, execution status, and action eligibility. It MUST shield callers from raw command types, `expected_revision`, execution marks, queue intent, and idempotency keys.

The CLI MUST submit the smallest supported sequence through the existing shared operator-command service, wait for each command record to settle, and return success only when the target is already admitted or the intended admission is accepted. It MUST recompute intent after bounded stale-revision conflicts, use a fresh idempotency identity when the typed command identity changes, and fail if the owner instance changes. It MUST fail closed for unknown, final, blocked, worktree-ineligible, active-run-limited, unsupported, or command-incapable targets without starting another owner or claiming admission. An idle-owner Start MUST NOT consume unrelated execution marks: the client MUST preserve them and return a typed operator-intent conflict if it cannot isolate the requested target through existing semantics. If the requested mark settles but Start does not, the client MUST return non-zero `partial_intent`, identify the remaining mark, warn that a later operator Start can consume it, and MUST NOT claim rollback. Every `partial_intent` result MUST list every command actually submitted by that invocation, regardless of whether that command settles successfully, and MUST omit commands that were skipped or rejected before submission.

#### Scenario: Failed submitted Start remains in the audit list

**Given**: the client submits `Start` and the owner settles it unsuccessfully
**When**: enqueue returns `partial_intent`
**Then**: `detail.commands_submitted` contains `start`
**And**: its order matches the command records actually submitted by this invocation

#### Scenario: Pre-existing mark is not reported as submitted

**Given**: `alpha` was already execution-marked before the client invocation
**When**: the client skips mark submission, submits Start, and Start fails
**Then**: `partial_intent.detail.commands_submitted` contains `start`
**And**: it does not contain `set_execution_mark`
**And**: the remaining mark and its later-consumption warning are still reported truthfully
