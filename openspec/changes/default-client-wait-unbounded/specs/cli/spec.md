## MODIFIED Requirements

### Requirement: Observation-only completion wait

`cflx client wait <change-id>` MUST observe one owner and repository until the requested change reaches a repository-verifiable terminal success, a typed unsuccessful terminal outcome, owner replacement, or an explicitly configured positive timeout. Omitting `--timeout` MUST select an unbounded operation duration, and every accepted timeout spelling whose value is exactly zero (for example `0`, `0s`, `0ms`) MUST select the same unbounded operation duration; positive values below the existing minimum and above the existing maximum MUST remain usage errors. A positive timeout MUST create one monotonic operation deadline that bounds initial observation, repeated observation, event/poll recovery, repository classification, and every local or remote Git subprocess. Positive deadline expiry MUST terminate and reap owned subprocesses, return typed `timeout`, and MUST NOT be replaced by a later inner transport or evidence error. An unbounded wait has no operation deadline to reach a Git child, so it MUST place a finite per-invocation deadline of its own on every local or remote Git subprocess; expiry of that inner deadline MUST terminate and reap the child and be handled as a recoverable or typed evidence condition, and MUST NOT produce the operation-level `timeout` outcome, which remains reserved for explicit positive timeouts. Unbounded operation duration MUST NOT disable per-request transport limits or process cleanup. It MUST use event streaming when available and authoritative multi-resource polling to recover from gaps. Reads MUST agree on `instance_id`; revision-bearing resources must reconcile at one `state_revision`, and `event_sequence` must not move backwards. `status` may end bounded rereads with typed observation conflict, while `wait` must keep reconciling until its configured positive deadline or a terminal outcome. API presentation and command records MAY provide progress but MUST NOT alone certify implementation or integration completion.

Wait MUST submit no mutation command. Change disappearance alone MUST NOT count as success. For terminal mode `merged`, success requires the existing repository completion oracle to return `Completed` for the captured base branch. For `base_published`, the selected remote base ref must additionally equal the locally verified base tip. For `branch_pushed`, archived proposal evidence must exist on the named local change branch and the selected remote branch ref must equal that local branch tip; this proves publication, not base integration. `NotCompleted`, `Contradictory`, `EvidenceError`, unsupported mode, and missing or ambiguous repository evidence MUST remain typed non-success outcomes.

#### Scenario: Wait proves successful local integration

**Given**: `alpha` is processed by one owner in local integration mode
**When**: archive completes and repository evidence proves the resulting change is integrated into the intended base
**Then**: `cflx client wait alpha --json` exits zero with outcome `completed`
**And**: it identifies the observed owner instance and repository completion evidence

#### Scenario: Wait proves pushed publication

**Given**: `alpha` is processed in `branch_pushed` terminal mode
**When**: an archived proposal exists on the named local change branch and the selected remote branch ref equals that local branch tip
**Then**: `cflx client wait alpha --json` exits zero with outcome `completed`
**And**: it reports branch publication without claiming base integration

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
**Then**: wait reevaluates current repository completion evidence once for `alpha`
**And**: it returns successful completion only if repository evidence alone proves it
**And**: otherwise it exits non-zero with outcome `owner_restarted` without inferring settlement of commands owned by the prior process

#### Scenario: Omitted timeout waits without an operation deadline

**Given**: `alpha` has not reached a terminal outcome
**When**: a caller runs `cflx client wait alpha` without `--timeout`
**Then**: wait continues observing without a synthesized operation deadline
**And**: bounded transport and Git subprocess safety remain active

#### Scenario: Zero timeout waits without an operation deadline

**Given**: `alpha` has not reached a terminal outcome
**When**: a caller runs `cflx client wait alpha --timeout 0`
**Then**: wait behaves the same as when timeout is omitted
**And**: it does not return an immediate `timeout`

#### Scenario: Stalled remote verification does not hang an unbounded wait

**Given**: an unbounded wait starts terminal-mode verification and the remote Git lookup does not complete
**When**: the finite per-invocation subprocess deadline expires
**Then**: the Git child is terminated and reaped
**And**: wait continues observing or returns a typed evidence outcome
**And**: it does not return the operation-level `timeout` outcome

#### Scenario: Timeout is not completion

**Given**: a positive configured wait duration expires before verified success or another terminal outcome
**When**: wait reaches the deadline
**Then**: it exits non-zero with outcome `timeout`
**And**: it performs no mutation while exiting

#### Scenario: Stalled owner read respects the operation deadline

**Given**: the owner accepts a UDS connection but does not complete a response
**When**: the positive configured wait deadline expires
**Then**: wait exits non-zero with outcome `timeout`
**And**: it does not wait for the transport's longer per-request timeout
**And**: it submits no mutation

#### Scenario: Stalled remote verification respects the operation deadline

**Given**: terminal-mode verification starts a remote Git lookup that does not complete
**When**: the positive configured wait deadline expires
**Then**: the Git child is terminated and reaped
**And**: wait exits non-zero with outcome `timeout`
**And**: no later repository evidence error replaces that outcome
