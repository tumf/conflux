## ADDED Requirements

### Requirement: Analyze uses proposal frontmatter dependencies

analyze フェーズは、proposal frontmatter に `dependencies` が存在する場合、それを依存関係 source として本文 `## Dependencies` セクションより優先して扱わなければならない（MUST）。frontmatter に `dependencies` が存在しない場合のみ、既存の本文 `## Dependencies` セクションを fallback として使わなければならない（MUST）。

#### Scenario: analyze prefers frontmatter dependencies over body section

**Given**: ある change の `proposal.md` frontmatter に `dependencies: ["base-change"]` がある
**And**: 本文 `## Dependencies` セクションには別の依存関係が書かれている
**When**: analyze フェーズが依存関係を解釈する
**Then**: `base-change` が依存関係 source として採用される
**And**: 本文 `## Dependencies` の値は fallback としてのみ扱われる

#### Scenario: analyze falls back to body dependencies when frontmatter is absent

**Given**: ある change の `proposal.md` に frontmatter `dependencies` がない
**And**: 本文 `## Dependencies` セクションがある
**When**: analyze フェーズが依存関係を解釈する
**Then**: 本文 `## Dependencies` の change id 一覧が依存関係 source として採用される

### Requirement: Analyze treats proposal priority as soft ordering hint

analyze フェーズは、proposal frontmatter の `priority` を dependency graph のハード制約として扱ってはならない（MUST NOT）。`priority` は、依存関係を壊さない範囲で `order` を決めるためのソフトヒントとしてのみ扱わなければならない（MUST）。

#### Scenario: priority influences order but not dependencies

**Given**: 2 つの独立した change があり、片方の frontmatter に `priority: high`、もう片方に `priority: low` がある
**When**: analyze フェーズが `order` と `dependencies` を決定する
**Then**: `priority: high` の change は `order` で先に置かれてよい
**And**: `dependencies` には priority だけを理由とした依存関係は追加されない

### Requirement: Analyze includes proposal references as context only

analyze フェーズは、proposal frontmatter の `references` を hard dependency source として扱ってはならない（MUST NOT）。`references` は analyze プロンプトまたは同等の解析コンテキストに補助情報として含めなければならない（MUST）。

#### Scenario: references are passed as analyze context

**Given**: ある change の `proposal.md` frontmatter に `references: ["src/analyzer.rs", "openspec/specs/parallel-analysis/spec.md"]` がある
**When**: analyze フェーズがその change の解析プロンプトを構築する
**Then**: `references` の値は解析コンテキストに含まれる
**And**: `references` だけを理由として依存関係は追加されない

### Requirement: Analyze tolerates unknown frontmatter key warnings

proposal metadata parser が unknown frontmatter key の warning を報告していても、analyze フェーズは解析を継続しなければならない（MUST）。warning は利用者に伝達してよいが、analyze を失敗させてはならない（MUST NOT）。

#### Scenario: analyze continues with unknown metadata warning

**Given**: ある change の `proposal.md` frontmatter に unknown key があり、metadata parser が warning を返している
**When**: analyze フェーズがその change を解析対象に含める
**Then**: analyze は継続される
**And**: known metadata は通常どおり利用される

> Canonical archive expectation: `parallel-analysis` capability は analyze フェーズでの proposal frontmatter metadata の利用規則を canonical spec として保持する。
