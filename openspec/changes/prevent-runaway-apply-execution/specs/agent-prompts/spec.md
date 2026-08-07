## MODIFIED Requirements

### Requirement: Apply system prompt MUST enforce non-interactive iteration

The Apply system prompt MUST explicitly state that the agent cannot ask questions and must make autonomous decisions under operational constraints. It MUST NOT interpret autonomy as permission for unbounded verification. Verification commands MUST run once by default. Re-execution of the identical command MUST be limited to at most three total executions within one Apply invocation and each retry MUST follow repository repair or concrete environment-recovery evidence. No-change stability loops MUST be prohibited. When bounded verification cannot complete or remains unstable, Apply MUST record structured `verification_timeout` or `verification_unstable` blocker facts and return control to Conflux rather than continue indefinitely.

#### Scenario: Continue repository work without asking questions

**Given**: Apply encounters repository-fixable uncertainty
**When**: a bounded implementation decision is available
**Then**: the agent does not ask the user a question
**And**: it makes the best repository-supported decision and proceeds
**And**: it remains subject to verification retry and invocation runtime limits

#### Scenario: Stability loop is prohibited

**Given**: a verification command has completed once
**And**: no repository repair or concrete environment recovery has occurred
**When**: the agent considers repeating the command only to prove stability
**Then**: Apply guidance prohibits the repetition
**And**: the agent records `verification_unstable` facts when flakiness prevents truthful completion

#### Scenario: Verification retry follows new evidence

**Given**: a verification command failed
**And**: the agent changed repository code or captured concrete environment-recovery evidence
**When**: the agent reruns the identical verification command
**Then**: the rerun counts toward a maximum of three total executions
**And**: reaching the limit requires blocker handoff rather than a fourth execution

### Requirement: Apply prompt MUST escalate implementation blockers

Apply guidance MUST distinguish repository-fixable work, mockable dependencies, non-repository external prerequisites, terminal rejection proposals, and bounded verification failures. When a required verification command cannot complete within the invocation budget, guidance MUST record a `verification_timeout` blocker. When verification remains nondeterministic after evidence-bearing retries, guidance MUST record a `verification_unstable` blocker. Both outcomes MUST carry concrete command, attempt, duration, output, repository-diff or recovery evidence, impact, unblock condition, next action, and resumability. They MUST NOT create `REJECTED.md` solely because verification timed out or was unstable.

#### Scenario: Apply records bounded verification timeout

**Given**: a required foreground verification cannot complete within the bounded Apply invocation
**When**: no repository-only repair can produce timely evidence
**Then**: tasks.md gains a narrative Implementation Blocker with category `verification_timeout`
**And**: stdout contains matching structured blocker facts
**And**: Apply returns control without leaving background verification running
**And**: it does not create `REJECTED.md`

#### Scenario: Apply records unstable verification

**Given**: the same verification has reached the evidence-bearing retry limit
**And**: results remain nondeterministic
**When**: Apply cannot truthfully mark the task complete
**Then**: it records category `verification_unstable` with all attempts and evidence
**And**: it stops rather than starting another stability loop
