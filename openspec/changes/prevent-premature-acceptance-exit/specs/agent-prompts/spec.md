## ADDED Requirements

### Requirement: Acceptance agents retain ownership of started verification

Acceptance guidance MUST require the parent acceptance agent to wait for the final result of every command, sub-agent, job, or monitored verification it starts before emitting its final verdict. The agent MUST NOT terminate with only progress prose, a waiting message, or a promise to decide after a future completion notification. This rule MUST remain portable and MUST NOT depend on a named runtime-specific monitoring tool.

#### Scenario: monitored verification completes before verdict

**Given**: the acceptance agent starts a verification operation whose result arrives asynchronously
**When**: the operation is still running or its completion notification has not been received
**Then**: the parent acceptance agent continues waiting and does not terminate the acceptance response
**And**: after the final result is received, the parent evaluates the evidence and emits the canonical acceptance verdict

#### Scenario: waiting prose is not a terminal acceptance response

**Given**: the acceptance agent has started a long-running verification
**When**: it reports that verification is still being monitored
**Then**: that status message is not treated by the guidance as completion
**And**: the agent remains responsible for obtaining the result and emitting the canonical verdict

#### Scenario: standard and SPECA skills share completion ownership

**Given**: either `cflx-accept` or `cflx-accept-with-speca` is selected
**When**: acceptance starts verification work
**Then**: both skills require the same wait-for-result and final-verdict behavior
**And**: neither skill depends on a provider-specific command or tool name
