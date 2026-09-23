## MODIFIED Requirements

### Requirement: Parallel execution acceptance loop

Managed-worktree execution SHALL run bounded Acceptance after successful Apply and before archive. Every configured frontend SHALL use the same in-memory execution manifest, externally cached declared-gate evidence, verdict parsing, missing-verdict retry, deadline finalization, history, restart, and stalled-hold behavior.

Before dispatch, Conflux MUST derive a versioned Acceptance execution manifest and SHA-256 input fingerprint from workspace file state, workspace Git state, and base-tree comparison. The manifest MUST bind the candidate commit/tree, review base/range, canonical change inputs, selected embedded Acceptance skill identity, and eligible declared change-blocking verification identities and evidence. The embedded skill and executable identities MAY change the fingerprint under the existing tool-identity precedent. Runtime-owned declared gate execution MUST precede semantic review. Any persisted manifest or gate evidence MUST live in Conflux-owned external state and MUST remain cache or diagnostics rather than workflow authority.

Declared gates and semantic review share the existing effective absolute Acceptance deadline. Conflux MUST reserve the final 30 seconds for termination, reap, external diagnostic persistence, and projection. A configured effective deadline of 300 seconds therefore provides 270 seconds for gates plus review. Gate execution consumes that shared budget; if it cannot finish before finalization, Acceptance enters the existing stalled execution-hold contract with category `acceptance_declared_gate_deadline_exhausted`. An external prerequisite preventing a declared gate MUST retain the existing resumable external-prerequisite stalled-hold contract and takes precedence over runtime/manifest-defect classification.

A completed canonical verdict yields `pass` or `fail`. A malformed manifest, unavailable external cache path, runtime defect, gate deadline exhaustion, reviewer deadline exhaustion, or unproven reviewer cleanup MUST yield status `stalled`, blocker kind `none`, `resumable: false`, and a stable acceptance category. These outcomes MUST settle before the outer command deadline. The outer runtime-limit `error` remains only a defect-containment fallback. Review-deadline termination MUST bypass missing-verdict retry. A non-resumable hold MAY suppress an unchanged retry only from typed state owned by the current live owner process; external state MUST NOT recreate that suppression after restart. After owner restart the prior hold is not reconstructed: the change projects from workspace state alone with no Acceptance hold, and the first admission in the new owner lifetime, whether explicit retry or automatic mark settlement, dispatches one bounded attempt. A later unchanged exhaustion is held again by live typed state.

#### Scenario: process restart reconstructs work without external authority

- **GIVEN** an acceptance protocol retry or bounded hold was active before process termination
- **AND** the managed workspace remains applied but unarchived
- **WHEN** Conflux restarts
- **THEN** it reconstructs Acceptance from workspace file and Git state
- **AND** external cache may reuse exact gate evidence but cannot refuse a fresh explicit retry or select the next action

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

#### Scenario: unchanged deadline exhaustion is suppressed only by the live owner

- **GIVEN** the current live owner's typed reducer state records a non-resumable Acceptance deadline hold for the unchanged input
- **WHEN** shared admission or mark settlement evaluates the change within that owner lifetime
- **THEN** it refuses admission as `unchanged_acceptance_input`
- **AND** it dispatches no analysis, Apply, gate, or Acceptance work

#### Scenario: restart permits one fresh bounded retry

- **GIVEN** a prior owner recorded a non-resumable Acceptance deadline hold
- **AND** the repository input is unchanged
- **WHEN** a restarted owner receives an explicit retry or automatic mark settlement evaluates the change
- **THEN** it admits one fresh bounded Acceptance attempt
- **AND** each still-valid externally cached gate result may be reused

#### Scenario: changed input permits one bounded retry

- **GIVEN** candidate revision/tree, review base/range, canonical change input, embedded skill/executable identity, declaration, automation, command/tool identity, or bound artifact changed since the live owner's hold
- **WHEN** explicit retry is requested
- **THEN** the changed fingerprint permits one retry edge
- **AND** each still-valid externally cached gate result is reused

#### Scenario: missing cache reruns without proving PASS

- **GIVEN** a declared gate was successful but its external cache was deleted or is unreadable
- **WHEN** Acceptance evaluates the unchanged repository input
- **THEN** it executes the declared gate again or returns its ordinary typed hold
- **AND** cache absence does not imply a successful gate or verdict

#### Scenario: target cleanliness has no Conflux exclusion

- **GIVEN** a target worktree contains staged, unstaged, or untracked content, or the legacy `.cflx/verification-evidence` path exists including as self-ignored content
- **WHEN** Acceptance evaluates cleanliness before or after a declared gate
- **THEN** it uses normal tracked/untracked porcelain status without an evidence-directory exception and separately probes only for the legacy path's existence
- **AND** Conflux creates no target-local cache, ignore file, manifest, envelope, or gate artifact
- **AND** unrelated ignored paths are not evaluated

#### Scenario: external state failure stops before gate execution

- **GIVEN** the resolved external Acceptance state path is unavailable, unwritable, or resolves inside the target repository or a managed worktree
- **WHEN** runtime prepares a declared gate
- **THEN** it emits a typed runtime hold before launching the gate
- **AND** it does not fall back to the target repository or worktree
