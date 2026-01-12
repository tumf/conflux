# parallel-analysis Specification

## Purpose
TBD - created by archiving change pass-approved-changes-to-analyzer. Update Purpose after archive.
## Requirements
### Requirement: 並列実行依存関係分析プロンプト

依存関係分析プロンプトは、選択済み（`is_approved = true`）の change を明確にマークし、各 change の proposal ファイルパスを明示すること SHALL。

プロンプトには以下を含むこと SHALL：
- 選択済み change を `[x]` でマーク
- 未選択 change を `[ ]` でマーク（将来の拡張性のため）
- 各 change の完全なファイルパス（`openspec/changes/{change_id}/proposal.md`）
- 「選択済み change のみ分析する」という明示的な指示

#### Scenario: 選択済みと未選択が混在する場合

- **GIVEN** 以下の change リスト:
  - `add-feature-a` (選択済み)
  - `add-feature-b` (選択済み)
  - `add-feature-c` (未選択)
- **WHEN** `ParallelizationAnalyzer::build_parallelization_prompt()` を呼び出す
- **THEN** プロンプトには以下が含まれること:
  ```
  [x] add-feature-a (openspec/changes/add-feature-a/proposal.md)
  [x] add-feature-b (openspec/changes/add-feature-b/proposal.md)
  [ ] add-feature-c (openspec/changes/add-feature-c/proposal.md)
  ```
- **AND** プロンプトに「Analyze ONLY the changes marked with [x]」という指示が含まれること

#### Scenario: 全て選択済みの場合

- **GIVEN** 全ての change が選択済み（`is_approved = true`）
- **WHEN** プロンプトを生成する
- **THEN** 全ての change に `[x]` マークが付くこと
- **AND** ファイルパスが各行に含まれること

#### Scenario: AIエージェントによる proposal ファイル読み取り

- **GIVEN** プロンプトに明示的なファイルパス（例: `openspec/changes/add-feature/proposal.md`）が含まれる
- **WHEN** AIエージェントがプロンプトを処理する
- **THEN** AIエージェントはパスを推測せずに直接ファイルを読み取れること
- **AND** `<change_id>` のようなプレースホルダーの解釈が不要であること

#### Scenario: 依存関係分析結果の形式は変更なし

- **GIVEN** プロンプトフォーマットが変更される
- **WHEN** AIエージェントが依存関係分析を実行する
- **THEN** レスポンス形式（JSON with `groups` array）は変更されないこと
- **AND** 既存の `parse_response()` ロジックが引き続き動作すること

