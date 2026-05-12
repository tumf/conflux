### Requirement: Three-Level Runtime State Model

Conflux SHALL define an in-memory runtime state model with explicit Orchestrator, Project, and Proposal layers.

The Orchestrator layer SHALL own global orchestration lifecycle and project aggregation only. The Project layer SHALL own project-local runtime lifecycle, proposal collection, dispatch view derivation, dependency-blocked view derivation, and base-lane ownership. The Proposal layer SHALL own one proposal/change lifecycle status.

The runtime model MUST NOT introduce out-of-worktree durable workflow-control state for resume routing, acceptance routing, archive routing, merge routing, or next-action decisions.

<!-- Expected canonical result after archive: `runtime-state` will define the Orchestrator > Project > Proposal hierarchy as the target runtime architecture while preserving constitution-compliant workspace/git-derived workflow routing. -->

#### Scenario: runtime state is hierarchical

**Given**: Conflux is representing runtime state for multiple projects
**When**: a runtime snapshot is created
**Then**: the snapshot contains an orchestrator layer
**And**: each project is nested under the orchestrator layer
**And**: each proposal is nested under its project layer
**And**: proposal lifecycle state is not stored directly at the orchestrator root

#### Scenario: runtime state is not durable workflow control

**Given**: a workspace has repository-visible state that determines the next action
**When**: the new runtime model is introduced
**Then**: no new out-of-worktree durable runtime status is required to choose resume, acceptance, archive, merge, or next-action routing
**And**: deleting external state such as `~/.local/state/cflx/**` does not change the next action for the same workspace file state, workspace git state, and base-branch tree comparison

### Requirement: Single Proposal Lifecycle Status

Each proposal runtime entry SHALL represent lifecycle state with exactly one `ProposalStatus` enum value.

The proposal runtime model SHALL NOT require simultaneous canonical queue intent, activity, wait state, and terminal fields to determine current lifecycle status. Compatibility views such as queued, stalled, merge-wait, resolve-wait, rejected, and merged MAY be derived from `ProposalStatus`, but they MUST NOT be stored as independent canonical lifecycle sets.

<!-- Expected canonical result after archive: `runtime-state` will establish single-enum proposal lifecycle status and forbid the old multi-axis lifecycle combination as the target model. -->

#### Scenario: proposal has one lifecycle status

**Given**: proposal `alpha` is represented in runtime state
**When**: `alpha` transitions from queued to applying
**Then**: `alpha` has status `Applying`
**And**: `alpha` does not also need a separate canonical queued flag, activity flag, wait flag, or terminal flag to describe the current lifecycle state

#### Scenario: terminal state is not regressed by stale event

**Given**: proposal `alpha` has status `Merged`
**When**: a stale apply, archive, merge-wait, or resolve-wait observation for `alpha` is reduced
**Then**: `alpha` remains `Merged`
**And**: derived views do not reintroduce `alpha` as queued, merge-wait, or resolve-wait

#### Scenario: rejected state remains final until explicit reactivation evidence

**Given**: proposal `alpha` has status `Rejected`
**When**: stale success or retry observations for `alpha` are reduced
**Then**: `alpha` remains `Rejected`
**And**: `alpha` is not selected as a dispatch candidate from derived project views

### Requirement: Project-Level Base Lane Ownership

The Project runtime layer SHALL model base-lane ownership as a project-level resource that serializes merge, resolve, and rejecting-review operations within a project.

A project MUST NOT expose a dispatch view that starts more than one base-lane owner simultaneously. Proposal-level statuses MAY indicate waiting for merge, resolve, or rejection review, but active base-lane ownership SHALL be represented at the Project layer.

<!-- Expected canonical result after archive: `runtime-state` will define base-lane ownership at project scope so scheduler loop conditions can be derived from one project-level lane owner. -->

#### Scenario: resolving owns the project base lane

**Given**: project `p1` has proposal `alpha` actively resolving
**When**: proposal `beta` becomes ready for rejecting review
**Then**: project `p1` keeps `alpha` as the active base-lane owner
**And**: `beta` is represented as waiting rather than active rejecting
**And**: the project dispatch view does not start both operations concurrently

#### Scenario: base lane clears after merge completion

**Given**: project `p1` has proposal `alpha` occupying the base lane for resolving or merge completion
**When**: `alpha` reaches `Merged`
**Then**: the project base lane becomes idle
**And**: the project dispatch view may select the next eligible merge, resolve, or rejecting-review proposal

### Requirement: Additive Runtime Model Introduction

The initial three-level runtime state change SHALL be additive and SHALL NOT replace existing execution paths, scheduler wiring, TUI/Web state consumption, server runner behavior, or obsolete serial-path removal in the same change.

Follow-up changes MAY migrate those consumers after the new model has reducer and snapshot test coverage.

<!-- Expected canonical result after archive: `runtime-state` will require the first migration step to add tested model/reducer primitives before rewiring execution surfaces. -->

#### Scenario: existing execution behavior remains unchanged

**Given**: the three-level runtime state model has been added
**When**: existing parallel execution, server, TUI, and legacy orchestration tests run
**Then**: they continue to use their existing runtime paths unless explicitly migrated by a later change
**And**: the new runtime model is covered by isolated reducer and snapshot tests

#### Scenario: serial mode does not shape the new model

**Given**: serial mode is obsolete
**When**: the new runtime types are introduced
**Then**: they do not require serial-specific terminal archive semantics as a foundational design constraint
**And**: any legacy serial compatibility remains outside the new three-level model or in later compatibility adapters
