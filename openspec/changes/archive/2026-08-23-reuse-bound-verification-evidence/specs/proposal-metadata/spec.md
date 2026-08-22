## ADDED Requirements

### Requirement: Verification evidence reuse is bound to current repository state

Conflux MAY reuse successful Apply verification evidence during Acceptance only from a versioned repository-local sidecar created by a Conflux-runtime-owned executor that directly supervised the declared process and bound the result to one declared verification ID. An Apply-agent-authored record MUST NOT be reusable. The record MUST include the full-length Apply commit object ID and tree ID, exact argv, repository-relative working directory, tracked automation-file blob identity, tool executable identity, start and end timestamps, exit code, evidence artifact content digest, and clean index/worktree state before and after execution. The designated Git-excluded runtime evidence directory MUST be ignored when comparing cleanliness and tree differences; no other difference from the bound Apply commit is permitted. Acceptance MUST compare every binding with current Git state and the current proposal declaration. A missing, malformed, stale, dirty, mismatched, unsuccessful, agent-authored, or unverifiable record MUST force command rerun and MUST NOT imply PASS. Reuse decisions MUST NOT depend on out-of-worktree durable state.

#### Scenario: Exact successful evidence is reused

**Given**: a runtime-authored repository-local evidence sidecar identifies one change-blocking verification
**And**: every commit/tree, command, cwd, automation blob, tool, artifact, exit, and clean-state binding matches the current worktree and declaration
**When**: Acceptance evaluates that verification
**Then**: Acceptance may reuse the successful result without executing the same command
**And**: output identifies the verification as reused

#### Scenario: Stale commit evidence reruns

**Given**: otherwise valid evidence is bound to a different commit or tree
**When**: Acceptance evaluates the current result
**Then**: it reruns the declared verification
**And**: it does not infer PASS from the stale record

#### Scenario: Command or tool mismatch reruns

**Given**: evidence argv, cwd, automation blob, or tool identity differs from the current declaration or executable
**When**: Acceptance evaluates the record
**Then**: it reruns the verification with an actionable mismatch reason

#### Scenario: Dirty capture is never reusable

**Given**: a record says the index or worktree was dirty at capture, or current cleanliness cannot be proven
**When**: Acceptance evaluates the record
**Then**: the record is ineligible for reuse
**And**: Acceptance reruns or reports the ordinary execution blocker

#### Scenario: Evidence sidecar does not invalidate its own binding

**Given**: the current commit and tree match the bound Apply result
**And**: the only worktree difference is inside the designated runtime evidence directory
**When**: Acceptance evaluates the runtime-authored sidecar
**Then**: the sidecar difference does not make the capture dirty or stale
**And**: any difference outside that directory still forces rerun

#### Scenario: Apply-authored evidence is not authority

**Given**: an envelope was written or modified without a matching runtime-supervised execution
**When**: Acceptance evaluates the envelope
**Then**: the envelope is ineligible for reuse
**And**: Acceptance reruns the declared verification

#### Scenario: Malformed evidence fails closed to rerun

**Given**: a record is missing a required field, uses a short commit ID, has an invalid digest, or references a missing artifact
**When**: Acceptance parses it
**Then**: the record does not satisfy verification
**And**: the declared command remains the source of truth

#### Scenario: Restart preserves only repository-derived authority

**Given**: Conflux restarts and external caches are absent
**When**: Acceptance reconstructs the next action
**Then**: it derives reuse eligibility from repository-local runtime evidence and current Git state alone
