## ADDED Requirements

### Requirement: Verification evidence reuse is bound to current repository state

Conflux MAY reuse successful Apply verification evidence during Acceptance only from a versioned tracked workspace record that is repository-verifiable and bound to one declared verification ID. The record MUST include full Apply commit or tree identity, exact argv, repository-relative working directory, tracked automation-file blob identity, tool executable identity, start and end timestamps, exit code, evidence artifact content digest, and clean index/worktree state at capture. Acceptance MUST compare every binding with current Git state and the current proposal declaration. A missing, malformed, stale, dirty, mismatched, unsuccessful, or unverifiable record MUST force command rerun and MUST NOT imply PASS. Reuse decisions MUST NOT depend on out-of-worktree durable state.

#### Scenario: Exact successful evidence is reused

**Given**: a tracked evidence record identifies one change-blocking verification
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

#### Scenario: Malformed evidence fails closed to rerun

**Given**: a record is missing a required field, uses a short commit ID, has an invalid digest, or references a missing artifact
**When**: Acceptance parses it
**Then**: the record does not satisfy verification
**And**: the declared command remains the source of truth

#### Scenario: Restart preserves only repository-derived authority

**Given**: Conflux restarts and external caches are absent
**When**: Acceptance reconstructs the next action
**Then**: it derives reuse eligibility from tracked workspace evidence and current Git state alone
