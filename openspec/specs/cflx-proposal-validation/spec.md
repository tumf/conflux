## Requirements

### Requirement: evidence-hint-matching

The `_EVIDENCE_HINTS` tuple used by `_has_repository_evidence_hint` must include hints for common build/test toolchains across Python, Node.js, Rust, and Go ecosystems so that verification notes citing runnable commands from these ecosystems are accepted.

#### Scenario: Node.js npm run command accepted

**Given**: A task with verification note `(verification: run npm run build -- succeeds)`
**When**: `cflx.py validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted (no error about missing repository-verifiable evidence)

#### Scenario: Rust cargo test command accepted

**Given**: A task with verification note `(verification: cargo test passes)`
**When**: `cflx.py validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted

#### Scenario: Go test command accepted

**Given**: A task with verification note `(verification: go test ./... passes)`
**When**: `cflx.py validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted

#### Scenario: Test directory path accepted

**Given**: A task with verification note `(verification: test/integration/auth.test.ts passes)`
**When**: `cflx.py validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted

#### Scenario: Existing Python hints still accepted

**Given**: A task with verification note `(verification: pytest tests/test_auth.py passes)`
**When**: `cflx.py validate <id> --strict --evidence error` is executed
**Then**: The verification note is accepted (backward compatible)

## Requirements

### Requirement: no-delta-marker-validation

Strict validation MUST accept a change that has a `specs/.no-delta` marker file and no spec delta directories. The `.no-delta` file declares that the change intentionally carries no spec modifications.

#### Scenario: Change with .no-delta marker passes strict validation

**Given**: A change directory contains `specs/.no-delta` and no subdirectories under `specs/`
**When**: `cflx.py validate <id> --strict` is executed
**Then**: Validation passes without spec delta errors

#### Scenario: .no-delta marker conflicts with existing spec deltas

**Given**: A change directory contains both `specs/.no-delta` and one or more spec delta subdirectories under `specs/`
**When**: `cflx.py validate <id> --strict` is executed
**Then**: Validation fails with an error indicating `.no-delta` conflicts with existing spec deltas

#### Scenario: No .no-delta and no spec deltas fails strict validation

**Given**: A change directory has no `specs/.no-delta` file and no spec delta subdirectories under `specs/`
**When**: `cflx.py validate <id> --strict` is executed
**Then**: Validation fails with an error indicating no spec deltas found (unchanged from current behavior)

## Requirements

### Requirement: change-directory-validity-filter

`cflx.py` の `list_changes()` および `_find_change_dir()` は、`proposal.md` が存在しないディレクトリを有効な change として扱ってはならない（MUST NOT）。invalid ディレクトリを検出した場合は stderr に警告を出力しなければならない（MUST）。

#### Scenario: proposal.md のないディレクトリが list から除外される

- **GIVEN** `openspec/changes/broken-dir/` が存在するが `proposal.md` を含まない
- **WHEN** `cflx.py list` を実行する
- **THEN** `broken-dir` は change 一覧に表示されない
- **AND** stderr に `broken-dir` に関する警告が出力される

#### Scenario: proposal.md のあるディレクトリは従来どおり表示される

- **GIVEN** `openspec/changes/valid-change/proposal.md` が存在する
- **WHEN** `cflx.py list` を実行する
- **THEN** `valid-change` は change 一覧に表示される

#### Scenario: _find_change_dir が invalid ディレクトリを返さない

- **GIVEN** `openspec/changes/ghost-dir/` が存在するが `proposal.md` を含まない
- **WHEN** `show ghost-dir` を実行する
- **THEN** change が見つからないエラーが返る
