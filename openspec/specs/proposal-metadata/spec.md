## Requirements

### Requirement: proposal.md frontmatter metadata

`openspec/changes/<change-id>/proposal.md` は YAML frontmatter を任意で持てなければならない（MAY）。frontmatter が存在しない proposal も有効でなければならない（MUST）。frontmatter を持つ場合、proposal tooling は本文より前の frontmatter ブロックを metadata として解釈できなければならない（MUST）。

#### Scenario: proposal with frontmatter metadata is accepted

**Given**: `proposal.md` が先頭に YAML frontmatter を持ち、その後に `# Change:` 見出しと本文セクションを持つ
**When**: proposal tooling または proposal-aware code path がその proposal を読む
**Then**: frontmatter は metadata として解釈される
**And**: 本文の title と本文セクションは従来どおり利用できる

#### Scenario: proposal without frontmatter remains valid

**Given**: `proposal.md` が YAML frontmatter を持たず、従来どおり `# Change:` 見出しから始まる
**When**: proposal tooling または proposal-aware code path がその proposal を読む
**Then**: proposal は有効な proposal として扱われる
**And**: title と本文セクションは従来どおり利用できる

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
