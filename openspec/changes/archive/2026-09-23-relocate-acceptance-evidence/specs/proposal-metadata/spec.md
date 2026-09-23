## MODIFIED Requirements

### Requirement: Verification evidence reuse is bound to current repository state

Conflux MAY reuse successful Apply or runtime-owned Acceptance verification evidence during Acceptance only from a versioned Conflux-owned external cache created by a Conflux-runtime-owned executor that directly supervised the declared process and bound the result to one declared verification ID. The cache MUST resolve outside the target repository and every managed worktree under the configured or platform-default Conflux state root, isolated by project, workspace, and change identity. An agent-authored record MUST NOT be reusable. The version-2 record MUST include the full-length candidate commit object ID and tree ID, exact review base and range, exact argv, repository-relative working directory, tracked automation-file blob identity, tool executable identity, start and end timestamps, exit code, store-relative evidence artifact path and content digest, and clean index/worktree state before and after execution. Acceptance MUST compare every binding with current Git state, the current proposal declaration, and the runtime-owned execution manifest while evaluating target cleanliness without Conflux-path exclusions. A missing, malformed, stale, dirty, mismatched, unsuccessful, agent-authored, deleted, or unverifiable cache record MUST force runtime-owned command execution or a typed hold and MUST NOT imply PASS. Absence or invalidity of cache causes rerun only and never changes the next workflow action. External records MUST remain cache and diagnostics only and MUST NOT control admission, retry, archive, merge, or next-action selection. An unavailable external store or a store resolving inside the target repository or any managed worktree MUST fail closed before command execution and MUST NOT fall back to a target-worktree path.

#### Scenario: Exact successful evidence is reused

**Given**: a runtime-authored external evidence cache record identifies one change-blocking verification
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

#### Scenario: Evidence cache does not alter target cleanliness

**Given**: the current commit and tree match the bound candidate result
**And**: exact runtime-authored evidence exists only in the external Conflux state root
**When**: Acceptance evaluates the runtime-authored record
**Then**: target cleanliness is evaluated without excluding any Conflux-owned path
**And**: any target difference still forces rerun or a typed hold

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

#### Scenario: Restart does not promote external cache to workflow authority

**Given**: Conflux restarts while external cache records are present, absent, or deleted
**When**: Acceptance reconstructs the next action
**Then**: repository and Git state determine the next workflow action
**And**: valid cache may reuse a gate result but cannot refuse admission or imply PASS

#### Scenario: Review range mismatch invalidates evidence

**Given**: otherwise valid evidence is bound to a different candidate revision, review base, or review range
**When**: Acceptance constructs the current execution manifest
**Then**: the evidence is not reusable
**And**: the runtime executes the declared verification or returns its ordinary typed hold

#### Scenario: Undeclared command has no blocking authority

**Given**: the execution manifest contains every eligible change-blocking declaration
**When**: a semantic reviewer proposes an undeclared repository-wide command
**Then**: that command is not started or stored as blocking evidence
**And**: its absence cannot prevent a verdict

#### Scenario: External state failure does not pollute the target

**Given**: the configured or derived Acceptance state path cannot be created or written
**When**: runtime prepares a declared gate
**Then**: Acceptance returns a typed runtime hold before launching the gate
**And**: it creates no fallback directory or file in the target repository or worktree

#### Scenario: Legacy target-local evidence is not hidden or migrated

**Given**: a target contains legacy `.cflx/verification-evidence` content
**When**: the new runtime evaluates cleanliness or evidence reuse
**Then**: it does not open, parse, migrate, rewrite, hide, or delete that content
**And**: a dedicated existence probe reports the path for explicit repository-owner cleanup without evaluating unrelated ignored paths
