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

The native `cflx openspec validate` implementation MUST enforce deterministic proposal authoring contracts such as structural validity, verification-note presence, supported evidence enum usage, spec delta target existence, and other repository-verifiable formatting rules. It MUST NOT infer implementation-task adequacy solely from wording heuristics about runtime behavior claims or whether tasks appear implementation-facing.

For strict validation, every `MODIFIED Requirements` and `REMOVED Requirements` block MUST target a requirement identity that exists in the corresponding canonical `openspec/specs/<capability>/spec.md` file. Missing targets MUST fail validation before archive promotion.

#### Scenario: validator rejects missing modified target before archive

**Given**: a change delta under `openspec/changes/alpha/specs/demo/spec.md` contains `## MODIFIED Requirements`
**And**: it includes `### Requirement: Missing Requirement`
**And**: canonical `openspec/specs/demo/spec.md` does not contain that requirement identity
**When**: `cflx openspec validate alpha --strict` is executed
**Then**: validation fails
**And**: the diagnostic says `MODIFIED target not found in canonical spec`
**And**: archive promotion is not required to discover the missing target

#### Scenario: validator rejects missing removed target before archive

**Given**: a change delta under `openspec/changes/alpha/specs/demo/spec.md` contains `## REMOVED Requirements`
**And**: it includes `### Requirement: Missing Requirement`
**And**: canonical `openspec/specs/demo/spec.md` does not contain that requirement identity
**When**: `cflx openspec validate alpha --strict` is executed
**Then**: validation fails
**And**: the diagnostic says `REMOVED target not found in canonical spec`
**And**: archive promotion is not required to discover the missing target

#### Scenario: added requirements do not require existing canonical targets

**Given**: a change delta under `openspec/changes/alpha/specs/demo/spec.md` contains `## ADDED Requirements`
**And**: it includes `### Requirement: New Requirement`
**And**: canonical `openspec/specs/demo/spec.md` does not contain that requirement identity
**When**: `cflx openspec validate alpha --strict` is executed
**Then**: validation does not fail because the added requirement lacks a canonical target

#### Scenario: archive gate reports the same missing target blocker

**Given**: a change delta contains a missing `MODIFIED` or `REMOVED` target
**When**: `cflx openspec validate alpha --archive-gate` is executed
**Then**: validation fails with the missing-target diagnostic before `cflx openspec archive alpha --yes` is needed

### Requirement: self-referential-final-validation-task-guard

The native `cflx openspec validate` implementation SHALL reject checkbox tasks that require final OpenSpec validation of the same change as implementation evidence. Final OpenSpec validation is an archive gate, not an implementation checkbox task.

The validator SHALL allow non-checkbox final validation guidance sections to mention the same validation command.

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

#### Scenario: ordinary repository evidence remains accepted

**Given**: an active change `alpha`
**And**: `openspec/changes/alpha/tasks.md` contains implementation checkbox tasks verified by runnable commands such as `cargo test`, `npm run`, `go test`, or source/test file paths
**When**: `cflx openspec validate alpha --strict --evidence error` is executed
**Then**: those ordinary verification notes remain accepted

### Requirement: archive-equivalent-validation-command

Conflux SHALL provide a documented local validation command whose failure policy matches archive readiness. Users and agents MUST be able to reproduce archive-blocking evidence findings before invoking archive.

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
