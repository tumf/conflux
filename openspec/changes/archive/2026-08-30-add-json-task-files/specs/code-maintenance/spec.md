## ADDED Requirements

### Requirement: Change task artifacts support one Markdown or JSON representation

Conflux MUST accept exactly one repository-local task artifact per active or archived change entry: `tasks.md` or versioned `tasks.json`. Every workflow phase that reads or mutates task state MUST use one shared format-discriminated resolver and semantic task contract.

The shared abstraction MUST preserve each existing resolution mode: comprehensive progress, active-only, archived, and workspace-local Acceptance mutation. If both supported filenames exist in the entry selected by the caller's resolution mode, or the selected JSON file is unreadable, malformed, unsupported, or semantically invalid, task-dependent workflow decisions MUST fail closed. A lower-priority location MUST NOT hide an invalid higher-priority task artifact, and mutation mode MUST NOT fall back to a base-tree entry.

#### Scenario: JSON-only change follows the normal workflow

**Given**: An active change entry contains a valid `tasks.json` and no `tasks.md`
**When**: Conflux validates, applies, accepts, archives, resolves, resumes, or displays the change
**Then**: Each phase reads the same JSON task artifact through the shared task-file contract
**And**: Progress, completion, runtime follow-up, and archive evidence have the same meaning as equivalent Markdown task state

#### Scenario: Existing Markdown change remains compatible

**Given**: A change entry contains `tasks.md` and no `tasks.json`
**When**: Conflux processes the change after JSON support is introduced
**Then**: Existing Markdown checkbox, section, acceptance follow-up, location fallback, and completion behavior remains unchanged

#### Scenario: Two task artifacts are ambiguous

**Given**: The entry selected by the caller's resolution mode contains both `tasks.md` and `tasks.json`
**When**: Any task-dependent progress, validation, mutation, archive, or merge decision is attempted
**Then**: Conflux reports an ambiguity error
**And**: Neither file wins by precedence
**And**: No task mutation or success decision occurs

#### Scenario: Invalid higher-priority JSON does not fall back

**Given**: A managed worktree active entry contains malformed `tasks.json`
**And**: A lower-priority base or archived entry contains a valid task file
**When**: Conflux resolves task state
**Then**: Resolution fails on the selected worktree artifact
**And**: The lower-priority artifact is not used to hide the error

#### Scenario: Acceptance mutation remains workspace-local

**Given**: A managed worktree has no active task artifact and has an archived JSON task artifact
**And**: The base tree has another task artifact for the same change
**When**: Acceptance records or cleans up current follow-up state
**Then**: Conflux mutates only the worktree archived `tasks.json`
**And**: It never selects or writes the base-tree artifact

### Requirement: JSON task state is versioned, verifiable, and safely mutable

`tasks.json` MUST declare schema version 1 and a task array whose entries have unique non-empty IDs, non-empty titles, and closed statuses. Only `completed` tasks count as completed, and an empty list MUST NOT authorize archive or merge.

The task array MUST contain only active `implementation` or `specification` tasks. Narrative data, including Final Validation, MUST remain outside the task array and MUST NOT contribute to progress. Each internal Acceptance finding MUST contribute one virtual task-gate item, completed only when remediation is claimed, preserving existing Markdown completion behavior.

Conflux-owned JSON mutations MUST use atomic same-directory replacement, MUST preserve unknown additive fields, and MUST keep runtime Acceptance findings structurally separate from ordinary implementation tasks. Runtime-owned finding identity and actionable payload MUST survive Apply remediation claims, evidence updates, process restart, and cleanup.

#### Scenario: JSON progress has deterministic completion semantics

**Given**: A valid `tasks.json` contains one `pending`, one `in_progress`, and one `completed` task
**When**: Conflux calculates task progress
**Then**: Progress is one completed of three total
**And**: The change is not archive- or merge-complete

#### Scenario: Open Acceptance finding blocks completion

**Given**: Every ordinary JSON task has status `completed`
**And**: The current Acceptance follow-up contains one internal finding whose remediation is not claimed
**When**: Conflux calculates progress or evaluates archive or merge authorization
**Then**: The finding contributes one incomplete virtual task-gate item
**And**: Archive and merge remain unauthorized

#### Scenario: Narrative Final Validation cannot self-complete

**Given**: A JSON task document declares archive validation in its narrative Final Validation field
**When**: Native strict validation and progress calculation run
**Then**: The narrative does not contribute a task or completion status
**And**: Representing Final Validation as an ordinary task is rejected

#### Scenario: Unsupported or ambiguous JSON fails closed

**Given**: A `tasks.json` has an unsupported schema version, duplicate task ID, blank required field, or unknown status
**When**: Conflux validates or gates the change
**Then**: It reports the exact task-file error
**And**: It does not project `0/0`, completion, archive readiness, or merge authorization

#### Scenario: Runtime Acceptance follow-up round-trips in JSON

**Given**: Acceptance records structured findings in `tasks.json`
**When**: Apply claims remediation and records evidence, the process restarts, and Acceptance later passes
**Then**: Finding identity and actionable payload are unchanged before PASS
**And**: Claimed remediation and evidence are retained
**And**: PASS removes only the runtime-owned current follow-up state
**And**: Unowned JSON extension fields remain present

#### Scenario: Archive evidence pairs the same task-file format

**Given**: Git diff removes `openspec/changes/<id>/tasks.json`
**When**: Conflux verifies the archive transition
**Then**: It requires `tasks.json` at the exact valid archived entry
**And**: A `tasks.md` addition, both basenames, a nested archive, or an unrelated change ID is rejected

### Requirement: Task-file paths remain repository-verifiable workflow evidence

Task-file selection and all task mutations MUST be derived from workspace and Git-visible active or archived change entries. Conflux MUST NOT add out-of-worktree task state or let observability caches determine progress, completion, acceptance, archive, or merge behavior.

#### Scenario: Restart recomputes the same task source

**Given**: A workspace has one supported task artifact at a valid active or archived entry
**And**: Local Conflux state and caches are deleted before restart
**When**: Conflux resumes the workspace
**Then**: It selects the same repository task artifact from workspace evidence
**And**: It computes the same next action and task progress
