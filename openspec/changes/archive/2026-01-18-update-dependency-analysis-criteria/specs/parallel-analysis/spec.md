## MODIFIED Requirements
### Requirement: 並列実行依存関係分析プロンプト

依存関係分析プロンプトは、選択済み（`is_approved = true`）の change を明確にマークし、各 change の proposal ファイルパスを明示すること SHALL。

プロンプトには以下を含むこと SHALL：
- 選択済み change を `[x]` でマーク
- 未選択 change を `[ ]` でマーク（将来の拡張性のため）
- 各 change の完全なファイルパス（`openspec/changes/{change_id}/proposal.md`）
- 「選択済み change のみ分析する」という明示的な指示
- 依存関係分析結果を `order`（依存関係を満たした上での推奨実行順序）と `dependencies` の両方で返すためのレスポンス指示
- `dependencies` は「片方の change が他方の成果物・仕様・APIに明確に依存し、それがないと成立しない場合」にのみ付与することを明示する
- `order` は優先度や実行効率の推奨順序であり、依存関係とは独立した概念であることを明示する

#### Scenario: 必須条件のみを依存関係として返す
- **GIVEN** 依存関係分析対象の change が複数存在する
- **AND** そのうち一方が他方の成果物・仕様・APIを必須条件として使用している
- **WHEN** 依存関係分析が実行される
- **THEN** `dependencies` には必須条件の関係のみが含まれる
- **AND** 優先度や順序の好みだけの関係は `dependencies` に含まれない
