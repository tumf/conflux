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

The native `cflx openspec validate` implementation MUST remain resilient when parsing behavior-task verification metadata from valid UTF-8 proposal files. Checkbox tasks MAY express verification ownership and evidence either inline as `(verification: ...)` or as an indented standalone `verification:` continuation line attached to the immediately preceding checkbox task. Validation MUST preserve UTF-8 character boundaries when reporting invalid task content so proposal validation returns structured findings instead of panicking.

#### Scenario: Standalone verification continuation line is accepted

- **GIVEN** a checkbox task line is followed by an indented standalone `verification: unit - ...` continuation line
- **WHEN** `cflx openspec validate <change-id> --strict --evidence warn` runs
- **THEN** the validator treats the continuation line as verification metadata for the preceding checkbox task
- **AND** evidence and ownership checks evaluate that verification text the same way they evaluate inline `(verification: ...)` notes

#### Scenario: Standalone verification line with multi-byte text does not panic

- **GIVEN** a checkbox task line is followed by an indented standalone `verification:` continuation line containing multi-byte UTF-8 text
- **AND** a legacy preview/reporting path would otherwise cut through a multi-byte character
- **WHEN** `cflx openspec validate <change-id> --strict --evidence warn` runs
- **THEN** validation does not panic
- **AND** it either accepts the verification metadata or emits a structured validation finding

#### Scenario: Invalid task content still reports findings on UTF-8 boundaries

- **GIVEN** a `tasks.md` line is still invalid task content after standalone verification parsing rules are applied
- **AND** the reported preview must be truncated for display
- **WHEN** `cflx openspec validate <change-id> --strict` runs
- **THEN** the reported preview is truncated on UTF-8 character boundaries
- **AND** validation returns a structured finding instead of a char-boundary panic
