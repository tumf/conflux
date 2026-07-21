## Requirements

### Requirement: evidence-hint-matching

The OpenSpec archive gate MUST evaluate repository-verifiable evidence and ownership markers from the complete task verification note, not from a truncated substring caused by parenthesized or backticked command/prose content inside the note. The evidence matcher MUST accept generic repository-evidence vocabulary used by Conflux diagnostics and proposal guidance when it appears with a valid verification ownership marker, including source-path, test-file, and runnable-command wording. The matcher MUST also accept common concrete repository artifact and build-command evidence such as Dockerfiles, TOML configuration files, and Docker build commands. Weak narrative notes without concrete repository evidence MUST remain rejected.

#### Scenario: Manual verification note contains source paths and runnable command

**Given**: an implementation change task line contains an inline `(verification: manual - ...)` note with source paths and a runnable `cflx openspec validate <id> --strict` command
**When**: `cflx openspec validate <id> --archive-gate` evaluates the task
**Then**: the archive gate accepts the task's verification evidence instead of reporting that repository-verifiable evidence is missing

#### Scenario: Verification note contains generic evidence vocabulary

**Given**: an implementation change task line contains a verification note with a valid ownership marker and generic repository evidence wording such as `source paths`, `test files`, or `runnable command`
**When**: `cflx openspec validate <id> --archive-gate` evaluates the task
**Then**: the archive gate accepts the note as repository-verifiable evidence

#### Scenario: Verification note contains common build artifacts and commands

**Given**: an implementation change task line contains a verification note with a valid ownership marker and evidence such as `Dockerfile`, `.toml`, or `docker build`
**When**: `cflx openspec validate <id> --archive-gate` evaluates the task
**Then**: the archive gate accepts the note as repository-verifiable evidence

#### Scenario: Verification note contains parenthesized command or prose content

**Given**: an inline verification note contains parenthesized or backticked command/prose segments before the repository evidence hint
**When**: the validator extracts the verification note
**Then**: extraction includes the full evidence-bearing note rather than stopping at the first inner closing parenthesis

#### Scenario: Missing or weak verification remains rejected

**Given**: an implementation change task has no verification note, lacks a recognized verification ownership marker, or lacks repository-verifiable evidence
**When**: the archive gate evaluates the task
**Then**: the archive gate continues to emit the appropriate strict validation finding

### Requirement: no-delta-marker-validation

Strict validation MUST accept a change that has a `specs/.no-delta` marker file and no spec delta directories. The `.no-delta` file declares that the change intentionally carries no spec modifications.

#### Scenario: Change with .no-delta marker passes strict validation

**Given**: A change directory contains `specs/.no-delta` and no subdirectories under `specs/`
**When**: `cflx openspec validate <id> --strict` is executed
**Then**: Validation passes without spec delta errors

#### Scenario: .no-delta marker conflicts with existing spec deltas

**Given**: A change directory contains both `specs/.no-delta` and one or more spec delta subdirectories under `specs/`
**When**: `cflx openspec validate <id> --strict` is executed
**Then**: Validation fails with an error indicating `.no-delta` conflicts with existing spec deltas

#### Scenario: No .no-delta and no spec deltas fails strict validation

**Given**: A change directory has no `specs/.no-delta` file and no spec delta subdirectories under `specs/`
**When**: `cflx openspec validate <id> --strict` is executed
**Then**: Validation fails with an error indicating no spec deltas found (unchanged from current behavior)

## Requirements

### Requirement: change-directory-validity-filter

`cflx openspec` の change 列挙および change 解決処理は、`proposal.md` が存在しないディレクトリを有効な change として扱ってはならない（MUST NOT）。invalid ディレクトリを検出した場合は stderr に警告を出力しなければならない（MUST）。

#### Scenario: proposal.md のないディレクトリが list から除外される

- **GIVEN** `openspec/changes/broken-dir/` が存在するが `proposal.md` を含まない
- **WHEN** `cflx openspec list` を実行する
- **THEN** `broken-dir` は change 一覧に表示されない
- **AND** stderr に `broken-dir` に関する警告が出力される

#### Scenario: proposal.md のあるディレクトリは従来どおり表示される

- **GIVEN** `openspec/changes/valid-change/proposal.md` が存在する
- **WHEN** `cflx openspec list` を実行する
- **THEN** `valid-change` は change 一覧に表示される

#### Scenario: _find_change_dir が invalid ディレクトリを返さない

- **GIVEN** `openspec/changes/ghost-dir/` が存在するが `proposal.md` を含まない
- **WHEN** `cflx openspec show ghost-dir` を実行する
- **THEN** change が見つからないエラーが返る

## Requirements

### Requirement: Bundled skills use native OpenSpec CLI commands

Conflux skill sources, active canonical specs, and repository-facing validator guidance MUST instruct agents and users to call native `cflx openspec` subcommands for list/show/validate/archive operations. The repository MUST NOT depend on `skills/cflx-proposal/scripts/cflx.py` as an executable validator contract once native CLI parity is complete.

#### Scenario: repository no longer retains cflx.py validator helper

- **GIVEN** the native Rust validator already covers the proposal validation contract used by active Conflux workflows
- **WHEN** repository validation and skill-distribution checks are executed
- **THEN** the repository does not contain `skills/cflx-proposal/scripts/cflx.py`
- **AND** proposal validation continues to work through `cflx openspec validate ...`
- **AND** no active canonical spec or skill-facing documentation instructs the user to execute `cflx.py`

### Requirement: Native validator owns behavior-centric proposal checks

The native `cflx openspec validate` implementation MUST enforce deterministic proposal authoring contracts, including structured verification declarations, without using natural-language inference as workflow-control authority. In strict and archive-gate modes, malformed declarations, missing required fields, duplicate IDs, invalid phase/owner relationships, empty required values, unsafe automation paths, and automation paths that do not identify an existing tracked repository regular file MUST fail validation with actionable diagnostics.

Implementation and hybrid proposals MUST declare at least one pre-integration verification. Spec-only proposals MAY omit verification declarations. A post-integration declaration MUST identify repository automation ownership, trigger, evidence location, rerun action, and prerequisites. Validation MUST NOT access a network, external API, or deployed target to validate any declaration.

#### Scenario: strict validation accepts a complete post-integration contract

**Given**: an implementation proposal has a pre-integration verification
**And**: it declares a post-integration verification whose automation path is a tracked repository workflow file
**And**: all required fields and phase/owner relationships are valid
**When**: `cflx openspec validate alpha --strict` runs
**Then**: verification declaration validation succeeds without accessing the external target

#### Scenario: strict validation rejects ownerless cyclic gate

**Given**: an implementation proposal requires an outcome available only after integration
**And**: its post-integration declaration omits repository automation ownership or a rerun action
**When**: strict validation runs
**Then**: validation fails before apply
**And**: the diagnostic identifies the missing declaration field

#### Scenario: unsafe automation path is rejected

**Given**: a verification declaration uses an absolute path, parent traversal, external symlink, missing file, or non-regular file as `automation`
**When**: strict or archive-gate validation runs
**Then**: validation fails with the offending verification ID and path

#### Scenario: natural-language phase inference is advisory only

**Given**: proposal prose appears to describe a different verification phase from its structured declaration
**When**: validation runs
**Then**: the structured phase remains authoritative
**And**: any prose-based diagnostic is advisory and cannot create or change workflow routing

#### Scenario: implementation proposal requires repository-verifiable pre-integration evidence

**Given**: an implementation or hybrid proposal has no pre-integration verification declaration
**When**: strict validation runs
**Then**: validation fails with guidance to declare repository-verifiable implementation evidence
**And**: a spec-only proposal without declarations remains valid

### Requirement: self-referential-final-validation-task-guard

The native `cflx openspec validate` implementation SHALL reject checkbox tasks that require final OpenSpec validation of the same change as implementation evidence. Final OpenSpec validation is an archive gate, not an implementation checkbox task.

The validator SHALL allow non-checkbox final validation guidance sections to mention the same validation command. Runtime-owned `## Acceptance #N Failure Follow-up` sections SHALL NOT be treated as implementation task declarations for self-referential final-validation or repository-evidence validation because they mirror acceptance findings rather than proposal-authored implementation tasks.

#### Scenario: same-change final validation checkbox fails

**Given**: an active change `alpha`
**And**: `openspec/changes/alpha/tasks.md` contains a checkbox task that asks the implementer to run or record `cflx openspec validate alpha`
**When**: `cflx openspec validate alpha --strict --evidence error` is executed
**Then**: validation fails
**And**: the diagnostic identifies the task as a self-referential final validation checkbox
**And**: the diagnostic tells the author to move final validation to a non-checkbox `Final Validation` section

#### Scenario: non-checkbox final validation section passes

**Given**: an active change `alpha`
**And**: `openspec/changes/alpha/tasks.md` contains a non-checkbox `## Final Validation` section mentioning `cflx openspec validate alpha --strict --evidence warn`
**And**: all implementation checkbox tasks have valid repository-verifiable evidence notes
**When**: `cflx openspec validate alpha --strict --evidence error` is executed
**Then**: validation passes without a self-referential final validation diagnostic

#### Scenario: runtime-owned acceptance follow-up is not revalidated as implementation tasks

**Given**: an active change `alpha`
**And**: `openspec/changes/alpha/tasks.md` contains a runtime-owned `## Acceptance #2 Failure Follow-up` section
**And**: that section mirrors an archive-gate or repository-fixable acceptance finding as a checkbox without a proposal verification declaration
**When**: `cflx openspec validate alpha --strict --evidence error` is executed
**Then**: validation does not emit self-referential final-validation or missing-verification diagnostics for that runtime-owned checkbox
**And**: ordinary implementation tasks outside the runtime-owned section remain subject to those diagnostics

#### Scenario: ordinary repository evidence remains accepted

**Given**: an active change `alpha`
**And**: `openspec/changes/alpha/tasks.md` contains implementation checkbox tasks verified by runnable commands such as `cargo test`, `npm run`, `go test`, or source/test file paths
**When**: `cflx openspec validate alpha --strict --evidence error` is executed
**Then**: those ordinary verification notes remain accepted

### Requirement: archive-equivalent-validation-command

Conflux SHALL provide a documented local validation command whose failure policy matches archive readiness. Users and agents MUST be able to reproduce archive-blocking evidence findings before invoking archive.

Validation errors and an invalid validation result MUST block archive. Advisory warnings, including archived dependency reference warnings, MUST NOT block archive when no validation error exists. The archive-equivalent validation command and actual archive operation MUST apply the same blocking distinction between errors and warnings.

#### Scenario: local archive gate fails on evidence findings

**Given**: an active change `alpha` has an evidence finding that would block archive
**When**: the documented archive-equivalent validation command is executed locally
**Then**: the command exits non-zero
**And**: it reports the same evidence finding that would block archive

#### Scenario: archive failure message includes local reproduction command

**Given**: archive fails because validation found evidence issues
**When**: Conflux reports the archive failure
**Then**: the message includes the local archive-equivalent validation command for the same change
**And**: the message preserves the specific validation finding instead of reporting only generic archive verification failure
