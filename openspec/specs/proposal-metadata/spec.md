## Requirements

### Requirement: proposal.md frontmatter metadata

`openspec/changes/<change-id>/proposal.md` MAY contain YAML frontmatter, and a proposal without frontmatter MUST remain readable. When frontmatter contains `verifications`, proposal tooling MUST parse it as an ordered list of structured verification declarations and MUST preserve the declarations when the proposal is read or archived. Explicit verification metadata MUST remain authoritative over natural-language phase hints.

#### Scenario: proposal with verification metadata is accepted

**Given**: `proposal.md` contains valid frontmatter with pre-integration and post-integration verification declarations
**When**: proposal-aware tooling reads the proposal
**Then**: both declarations are retained with their original IDs, phases, owners, paths, evidence locations, rerun actions, and prerequisites
**And**: the proposal body remains available unchanged

#### Scenario: legacy proposal remains readable

**Given**: `proposal.md` has no frontmatter or no `verifications` field
**When**: tolerant proposal tooling reads it
**Then**: the proposal remains readable
**And**: no verification declaration is invented from prose

#### Scenario: archived proposal preserves declarations

**Given**: an active proposal contains valid verification declarations
**When**: the change is archived through the native archive command
**Then**: the archived `proposal.md` retains declarations equivalent to the active proposal

### Requirement: proposal priority field

frontmatter の `priority` フィールドは proposal の優先度を表し、`high`、`medium`、`low` のいずれかでなければならない（MUST）。

#### Scenario: valid priority values are accepted

**Given**: `proposal.md` frontmatter に `priority: high` が含まれる
**When**: proposal metadata が解析される
**Then**: `priority` は有効な metadata として保持される

### Requirement: proposal dependencies field with backward compatibility

frontmatter の `dependencies` フィールドは change id の配列でなければならない（MUST）。`dependencies` が frontmatter に存在する場合、proposal tooling は本文 `## Dependencies` より frontmatter を優先しなければならない（MUST）。frontmatter に `dependencies` が存在しない場合、既存 proposal との後方互換のため本文 `## Dependencies` セクションを引き続き解釈しなければならない（MUST）。

#### Scenario: frontmatter dependencies override body section

**Given**: `proposal.md` frontmatter に `dependencies: ["base-change"]` がある
**And**: 本文 `## Dependencies` セクションには別の依存関係が書かれている
**When**: proposal dependencies を解析する
**Then**: `base-change` が依存関係として採用される
**And**: 本文 `## Dependencies` の値では上書きされない

#### Scenario: body dependencies remain supported without frontmatter field

**Given**: `proposal.md` に frontmatter の `dependencies` がない
**And**: 本文に `## Dependencies` セクションがある
**When**: proposal dependencies を解析する
**Then**: 本文 `## Dependencies` の change id 一覧が依存関係として採用される

#### Scenario: author guidance for archived dependency references

**Given**: active proposal の `dependencies` に含まれる change が archive に移動済みである
**When**: author が dependency metadata を見直す
**Then**: author は archived dependency reference が queued dependency と同一ではないことを認識できる
**And**: runtime/validation が archived reference を dedicated diagnostics で区別報告する前提で、依存が既に充足済みかどうかを proposal metadata から判断して更新可否を決められる

### Requirement: unknown frontmatter keys produce warnings

frontmatter に既知ではない key が含まれていても、proposal tooling は proposal の読み取りを失敗させてはならない（MUST NOT）。既知ではない key は warning として報告されなければならない（MUST）。

#### Scenario: unknown frontmatter key is warned but accepted

**Given**: `proposal.md` frontmatter に既知キーではない `owner: tumf` が含まれる
**When**: proposal metadata を解析または検証する
**Then**: proposal の読み取りは成功する
**And**: `owner` が unknown key である warning が報告される

### Requirement: proposal references field

frontmatter の `references` フィールドは文字列配列でなければならず、関連ファイル、spec、change id、その他の参照先を表現できなければならない（MUST）。proposal tooling は `references` をそのまま保持し、失われないように扱わなければならない（MUST）。

#### Scenario: references list preserves multiple targets

**Given**: `proposal.md` frontmatter に `references: ["src/openspec.rs", "openspec/specs/spec-only-changes/spec.md", "add-base-capability"]` がある
**When**: proposal metadata を解析する
**Then**: 3 件すべての reference が順序を保って保持される

> Canonical archive expectation: `proposal-metadata` capability は proposal frontmatter の形式・意味・後方互換ルールを canonical spec として保持する。

### Requirement: proposal verification declarations

Each verification declaration MUST contain a unique non-empty `id`, a non-empty `requirement`, a `phase`, an `owner`, a non-empty `trigger`, a safe repository-relative `automation` path, a non-empty `evidence` location, a non-empty `rerun` action, and a `prerequisites` string list. `phase` MUST be `pre-integration` or `post-integration`. A pre-integration declaration MUST use `owner: conflux-acceptance`; a post-integration declaration MUST use `owner: repository-automation`.

#### Scenario: pre-integration declaration identifies repository verification

**Given**: an implementation proposal declares `phase: pre-integration`
**When**: metadata is parsed
**Then**: the declaration identifies Conflux acceptance ownership and a tracked repository automation file

#### Scenario: post-integration declaration identifies operational ownership

**Given**: a proposal declares `phase: post-integration`
**When**: metadata is parsed
**Then**: the declaration identifies repository automation ownership, its trigger, evidence location, rerun action, and external prerequisites

#### Scenario: explicit phase wins over contradictory prose

**Given**: a structured declaration says `phase: post-integration`
**And**: proposal prose could be interpreted as pre-integration
**When**: tooling classifies the verification phase
**Then**: it uses `post-integration`
**And**: prose analysis may emit an advisory warning but cannot change routing semantics
