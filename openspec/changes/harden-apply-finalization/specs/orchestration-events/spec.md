## ADDED Requirements

### Requirement: Apply commit presentation MUST use an explicit ephemeral event

Conflux MUST expose final Apply commit presentation through an execution event that explicitly identifies the target change, commit phase, and attempt. Reducers MAY retain the phase in process memory for TUI and additive API presentation, but the canonical lifecycle MUST remain `applying`. Commit presentation MUST NOT be persisted or used for scheduler eligibility, resume routing, acceptance, archive, merge, or next-action decisions.

#### Scenario: Finalization changes TUI presentation to commit

**Given**: a change is in the canonical Applying activity
**When**: finalization starts stage checking and the verified commit sequence
**Then**: an explicit commit-phase event identifies the change and attempt
**And**: the TUI renders `[commit]` without changing the canonical `applying` status

#### Scenario: Repair iteration restores apply presentation

**Given**: a final commit hook or stage cleanliness gate requires Apply repair
**When**: the next Apply iteration starts
**Then**: commit presentation is cleared
**And**: the TUI renders `[apply]`

#### Scenario: Completion and failure do not leave stale commit presentation

**Given**: commit presentation is active
**When**: finalization completes, fails, or is cancelled
**Then**: the reducer clears commit presentation
**And**: subsequent rendering does not retain stale `[commit]` state

#### Scenario: Restart ignores commit presentation

**Given**: a process stops while commit presentation is active
**When**: Conflux starts again
**Then**: routing is derived from workspace files and Git state
**And**: absence of the prior process-local commit phase does not change the next action
