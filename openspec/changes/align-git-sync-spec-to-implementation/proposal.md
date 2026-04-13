---
change_type: spec-only
priority: high
dependencies: []
references:
  - openspec/specs/git-sync/spec.md
  - src/server/api/git_sync.rs
  - src/config/types.rs
---

**Change Type**: spec-only

# Change: git-sync spec を実装に合わせて一本化し、エージェント起動の事前判断を明文化する

## Why

`openspec/specs/git-sync/spec.md` 内に相互矛盾する 2 つの `Requirement: git/sync must only run reconciliation when needed before push` が併記されている：

- **要件 A (L2-28)**: **pull フェーズ後**に収集した `local_sha` と `remote_sha` を比較
- **要件 B (L31-62)**: **pull フェーズ前**の local SHA と pull フェーズ後の remote SHA を比較

実装 (`src/server/api/git_sync.rs` の `plan_sync()` L181-188 と呼び出し元 L491-499) は **post-pull の両 SHA を比較** しており、要件 A と一致する。要件 B は実装と不一致の古い版であり、archive されずに spec に残っている。

また、`resolve_command` は AI エージェントを起動する高コスト処理であるため、push 試行失敗からの事後検知ではなく SHA 比較での **事前判断** で起動可否を決定する必要がある（実装は既にこの設計）。この設計原則が spec 上で明文化されていない。

## What Changes

- 実装と不一致の要件 B（pre-pull vs post-pull 比較）を削除
- 要件 A を canonical として残し、以下を追加明記：
  - `resolve_command` は AI エージェントを起動するため、push 試行前の SHA 比較で起動可否を **事前判断** しなければならない（MUST）
  - Scenario: `resolve_command invocation is decided before agent startup`
- 実装参照（`src/server/api/git_sync.rs` L181-188 / L491-499）と推奨設定（トップレベル `resolve_command` 必須、`server.resolve_command` 廃止）を Requirement 本文に追加
- `bare repo is newly cloned` Scenario を実装ルール（両 SHA が非空で一致した場合のみ skip）に沿って書き直す
- `## Purpose` セクションを追加

## Impact

- Affected specs: `git-sync`
- Affected code: なし（実装は既に `plan_sync()` で post-pull 比較と agent 起動事前判断を行っている）
