## MODIFIED Requirements

### Requirement: Observation-only completion wait

`cflx client wait <change-id>` MUST observe one owner and repository until the requested change reaches a repository-verifiable terminal success, a typed unsuccessful terminal outcome, owner replacement, or an explicitly configured positive timeout. Omitting `--timeout` MUST select an unbounded operation duration, and every accepted timeout spelling whose value is exactly zero (for example `0`, `0s`, `0ms`) MUST select the same unbounded operation duration; positive values below the existing minimum and above the existing maximum MUST remain usage errors. A positive timeout MUST create one monotonic operation deadline that bounds initial observation, repeated observation, event/poll recovery, repository classification, and every local or remote Git subprocess. Positive deadline expiry MUST terminate and reap owned subprocesses, return typed `timeout`, and MUST NOT be replaced by a later inner transport or evidence error. An unbounded wait has no operation deadline to reach a Git child, so it MUST place a finite per-invocation deadline of its own on every local or remote Git subprocess; expiry of that inner deadline MUST terminate and reap the child and be handled as a recoverable or typed evidence condition, and MUST NOT produce the operation-level `timeout` outcome, which remains reserved for explicit positive timeouts. Unbounded operation duration MUST NOT disable per-request transport limits or process cleanup. It MUST use event streaming when available and authoritative multi-resource polling to recover from gaps. Reads MUST agree on `instance_id`; revision-bearing resources must reconcile at one `state_revision`, and `event_sequence` must not move backwards. `status` may end bounded rereads with typed observation conflict, while `wait` must keep reconciling until its configured positive deadline or a terminal outcome. API presentation and command records MAY provide progress but MUST NOT alone certify implementation or integration completion.

On positive deadline expiry, the typed timeout detail MUST report `timeout_ms`, measured `wait_elapsed_ms`, a stable `timeout_stage`, and `commands_submitted: 0`. `timeout_stage` MUST distinguish `initial_observation`, `observing_owner`, `repository_certification`, and `remote_verification`. If a coherent observation of the target completed before expiry, detail MUST carry that latest completed target-only `last_observation`, including observation identity/cursor facts, the target change's existing sanitized progress projection, and its matching execution projection. The client MUST NOT include unrelated changes, unrestricted logs, or a mixed-revision projection. If no coherent observation completed, `last_observation` MUST be `null` and the client MUST NOT invent an owner identity. Wait MUST NOT start another read after deadline expiry merely to enrich diagnostics.

Wait MUST submit no mutation command. Change disappearance alone MUST NOT count as success. The display statuses `not queued`, `queued`, `applying`, `accepting`, `rejecting`, `archiving`, and `resolving` MUST continue observing. A `blocked` row without a structured external blocker, including a dependency wait, MUST continue observing. A `blocked` row whose structured blocker kind is `external` MUST release immediately with outcome `change_requires_action`. The statuses `error`, `merge wait`, `stopped`, and `stalled` MUST also release immediately with outcome `change_requires_action`; `rejected` MUST retain `change_rejected`. This classification MUST run on the initial observation and every later coherent observation.

For terminal mode `merged`, success requires the existing repository completion oracle to return `Completed` for the captured base branch. For `base_published`, the selected remote base ref must additionally equal the locally verified base tip. For `branch_pushed`, archived proposal evidence must exist on the named local change branch and the selected remote branch ref must equal that local branch tip; this proves publication, not base integration. On an observed `merged` row whose first certification returns `NotCompleted`, wait MUST perform one bounded coherent re-observation and re-certification to avoid reporting in-flight publication as failure. If that second certification remains `NotCompleted`, wait MUST release with `change_requires_action` rather than hold indefinitely. `Contradictory`, `EvidenceError`, unsupported mode, and missing or ambiguous repository evidence MUST remain their existing typed non-success outcomes.

#### Scenario: Timeout returns the latest coherent target observation

**Given**: a positive wait has coherently observed `alpha` in Acceptance
**And**: the observation includes target progress and matching execution facts
**When**: the wait deadline expires while the owner has not reached a typed terminal outcome
**Then**: wait exits non-zero with outcome `timeout`
**And**: detail reports the configured and measured durations, `timeout_stage: observing_owner`, and zero submitted commands
**And**: `last_observation` contains only `alpha`, its matching execution identity/state/phases/timing, and existing sanitized activity and log projections
**And**: no post-deadline read or workflow mutation occurs

#### Scenario: Timeout before the first coherent observation invents no state

**Given**: the owner does not complete the initial coherent observation
**When**: the positive wait deadline expires
**Then**: detail reports `timeout_stage: initial_observation`
**And**: `last_observation` is `null`
**And**: the envelope does not claim an unobserved owner instance

#### Scenario: Repository verification timeout remains distinguishable

**Given**: wait observed a settled row and began repository certification
**When**: the operation deadline expires during local or remote Git verification
**Then**: wait returns outcome `timeout` with stage `repository_certification` or `remote_verification` as applicable
**And**: the owned Git child is terminated and reaped
**And**: a later evidence or transport result does not replace the timeout outcome
**And**: the last observation remains the one completed before certification began
