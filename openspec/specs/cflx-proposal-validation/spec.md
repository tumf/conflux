## Requirements

### Requirement: evidence-hint-matching

The `_EVIDENCE_HINTS` tuple used by `_has_repository_evidence_hint` must include hints for common build/test toolchains across Python, Node.js, Rust, and Go ecosystems so that verification notes citing runnable commands from these ecosystems are accepted.

#### Scenario: Node.js npm run command accepted

**Given**: A task with verification note `(verification: run npm run build -- succeeds)`
**When**: `cflx openspec validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted (no error about missing repository-verifiable evidence)

#### Scenario: Rust cargo test command accepted

**Given**: A task with verification note `(verification: cargo test passes)`
**When**: `cflx openspec validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted

#### Scenario: Go test command accepted

**Given**: A task with verification note `(verification: go test ./... passes)`
**When**: `cflx openspec validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted

#### Scenario: Test directory path accepted

**Given**: A task with verification note `(verification: test/integration/auth.test.ts passes)`
**When**: `cflx openspec validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted

#### Scenario: Existing Python hints still accepted

**Given**: A task with verification note `(verification: pytest tests/test_auth.py passes)`
**When**: `cflx openspec validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted (backward compatible)

## Requirements

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

The native `cflx openspec validate` implementation MUST enforce deterministic proposal authoring contracts such as structural validity, verification-note presence, supported evidence enum usage, and other repository-verifiable formatting rules. It MUST NOT infer implementation-task adequacy solely from wording heuristics about runtime behavior claims or whether tasks appear implementation-facing.

#### Scenario: validator does not emit runtime-behavior wording heuristic

- **GIVEN** an implementation or hybrid proposal claims runtime behavior changes
- **AND** its task wording may or may not look implementation-facing
- **WHEN** `cflx openspec validate <change-id> --strict --evidence warn` runs
- **THEN** validation does not emit a finding based solely on heuristic inference that runtime behavior lacks implementation-facing tasks
- **AND** any remaining findings come from deterministic authoring-contract checks rather than acceptance-style quality judgment

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
