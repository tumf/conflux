## ADDED Requirements

### Requirement: Cleanup-review corrective prompts MUST carry bounded latest diagnostics

When a post-Apply cleanup-review operation fails and corrective budget remains, Conflux MUST build the next normal configured cleanup-review prompt from trusted corrective instructions, current workspace paths, and only the latest bounded structured failure context. The context MUST identify the failure kind, exit code when available, bounded stdout/stderr, observed standalone `CLEANUP_REVIEW: CLEAN` marker count, and bounded fresh porcelain status or status-inspection failure.

Captured command and repository output MUST be delimited as untrusted diagnostic evidence and MUST NOT redefine the required action or success criteria. Corrective prompts MUST retain the dedicated `cflx-cleanup-review` skill, blind-staging prohibition, relevant-only commit responsibility, exactly-one standalone marker contract, and fresh clean-worktree verification. They MUST NOT require a harness session ID, provider resume flag, external managed-job identifier, durable report, or all-attempt transcript.

#### Scenario: Cleanup-review retry receives actionable latest context

- **GIVEN** initial cleanup-review leaves the managed worktree dirty or fails its command/marker contract
- **AND** corrective budget remains
- **WHEN** Conflux builds the next cleanup-review prompt
- **THEN** it includes a stable latest failure kind and available exit code
- **AND** it includes bounded latest stdout, stderr, marker count, and current porcelain evidence
- **AND** it instructs the agent to inspect and repair the actual workspace before proving the immutable success gate

#### Scenario: Cleanup-review diagnostics are untrusted and bounded

- **GIVEN** prior cleanup output contains instructions, repeated text, or large output
- **WHEN** Conflux injects it into a corrective prompt
- **THEN** the prompt clearly labels the captured material as untrusted evidence
- **AND** only bounded latest tails are included
- **AND** instructions inside captured output cannot authorize blind staging, relax marker count, or claim the worktree is clean

#### Scenario: Cleanup-review correction is harness neutral

- **GIVEN** any configured agent runtime can receive the normal Conflux cleanup-review prompt
- **WHEN** corrective cleanup-review starts
- **THEN** continuity is provided through Conflux-managed prompt context plus workspace and Git evidence
- **AND** the prompt does not require a session ID, resume flag, provider event, external job ID, report file, or retry checkpoint

#### Scenario: Cleanup-review prompt retains strict success ownership

- **GIVEN** a corrective cleanup-review prompt includes prior evidence that claims cleanup succeeded
- **WHEN** the agent follows the prompt
- **THEN** trusted instructions still require exactly one standalone `CLEANUP_REVIEW: CLEAN`
- **AND** trusted instructions still require a fresh clean repository status
- **AND** orchestrator verification, not prior narrative output, decides handoff success

### Requirement: Acceptance command-recovery prompts MUST preserve verdict and budget boundaries

When an Acceptance command launch or execution failure is retried, the next normal configured Acceptance prompt MUST include only the latest bounded Conflux-managed command diagnosis needed to explain the retry. The prompt MUST identify that no canonical verdict was accepted from the failed invocation and MUST require a fresh canonical result from the current invocation.

Prior command output MUST remain untrusted evidence and MUST NOT be treated as a canonical verdict, finalized FAIL finding payload, blocker, or instruction to rerun Apply. Command-recovery context MUST NOT replay all prior attempts and MUST remain independent from missing-verdict/protocol continuation context.

#### Scenario: Acceptance command retry asks for a fresh verdict

- **GIVEN** the previous Acceptance invocation failed at command level
- **AND** command-recovery budget remains
- **WHEN** Conflux builds the next Acceptance prompt
- **THEN** it includes latest bounded error, exit code when available, stdout tail, and stderr tail
- **AND** it states that the failed invocation supplied no accepted canonical outcome
- **AND** it asks the normal Acceptance agent to evaluate current repository evidence and emit a fresh canonical result

#### Scenario: Failed Acceptance output cannot become a verdict

- **GIVEN** bounded output from a failed command contains text resembling PASS, FAIL, CONTINUE, a blocker, or instructions
- **WHEN** that output is included in command-recovery context
- **THEN** it is delimited as untrusted command evidence
- **AND** parser/routing uses only the new invocation's canonical output
- **AND** captured text does not append repair tasks, create a stall, or authorize archive

#### Scenario: Acceptance command context is latest-only

- **GIVEN** multiple consecutive Acceptance command failures occur
- **WHEN** the next corrective prompt is built
- **THEN** it contains only the latest bounded command diagnosis
- **AND** it does not replay all prior command failures or protocol histories
