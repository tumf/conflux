## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Managed-worktree execution SHALL run bounded Acceptance after successful Apply and before archive. Every configured frontend SHALL use the same execution manifest, declared-gate execution and reuse, verdict parsing, missing-verdict retry, deadline finalization, history, restart, and stalled-hold behavior.

Before dispatch, Conflux MUST derive a versioned Acceptance execution manifest and SHA-256 input fingerprint from workspace file state, workspace Git state, and base-tree comparison. The manifest MUST bind the candidate commit/tree, review base/range, canonical change inputs, selected embedded Acceptance skill identity, and eligible declared change-blocking verification identities and evidence. The embedded skill and executable identities MAY change the fingerprint under the existing tool-identity precedent; they MUST NOT be stored as out-of-worktree workflow authority. Runtime-owned declared gate execution MUST precede semantic review.

Declared gates and semantic review share the existing effective absolute Acceptance deadline. Conflux MUST reserve the final 30 seconds for termination, reap, persistence, and projection. A configured effective deadline of 300 seconds therefore provides 270 seconds for gates plus review. Gate execution consumes that shared budget; if it cannot finish before finalization, Acceptance enters the existing stalled execution-hold contract with category `acceptance_declared_gate_deadline_exhausted`. An external prerequisite preventing a declared gate MUST retain the existing resumable external-prerequisite stalled-hold contract and takes precedence over runtime/manifest-defect classification.

A completed canonical verdict yields `pass` or `fail`. A malformed manifest, runtime defect, gate deadline exhaustion, reviewer deadline exhaustion, or unproven reviewer cleanup MUST yield status `stalled`, blocker kind `none`, `resumable: false`, and a stable acceptance category. These outcomes MUST settle before the outer command deadline. The outer runtime-limit `error` remains only a defect-containment fallback. Review-deadline termination MUST bypass missing-verdict retry.

#### Scenario: process restart uses workspace evidence

- **GIVEN** an acceptance protocol retry or bounded hold was active before process termination
- **AND** the managed workspace remains applied but unarchived
- **WHEN** Conflux restarts
- **THEN** it reconstructs Acceptance from workspace file and Git state
- **AND** it does not require an external retry checkpoint

#### Scenario: focused declaration bounds Acceptance

- **GIVEN** a change declares one eligible repository-local change-blocking verification
- **AND** runtime executes or exactly reuses its successful result
- **WHEN** semantic Acceptance reviews the candidate and exact review range
- **THEN** no undeclared verification is required or started for completion
- **AND** Acceptance settles from declared evidence and semantic diff review

#### Scenario: reviewer exhausts its review deadline

- **GIVEN** declared gates have typed results
- **AND** semantic review has no complete verdict when finalization begins
- **WHEN** runtime terminates and reaps the reviewer
- **THEN** the change becomes `stalled` with category `acceptance_review_deadline_exhausted` before the absolute deadline
- **AND** missing-verdict retry is not admitted

#### Scenario: unchanged deadline exhaustion is not redispatched

- **GIVEN** the current fingerprint matches a prior non-resumable Acceptance deadline hold
- **WHEN** shared admission or mark settlement evaluates the change
- **THEN** it refuses admission as `unchanged_acceptance_input`
- **AND** it dispatches no analysis, Apply, gate, or Acceptance work

#### Scenario: changed input permits one bounded retry

- **GIVEN** candidate revision/tree, review base/range, canonical change input, embedded skill/executable identity, declaration, automation, command/tool identity, or bound artifact changed
- **WHEN** explicit retry is requested
- **THEN** the changed fingerprint permits one retry edge
- **AND** each still-valid gate result is reused
