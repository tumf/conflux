## MODIFIED Requirements

### Requirement: 並列実行依存関係分析プロンプト

依存関係分析プロンプトは、選択済み（`is_approved = true`）の change を明確にマークし、各 change の proposal ファイルパスを明示すること SHALL。

プロンプトには以下を含むこと SHALL：
- 選択済み change を `[x]` でマーク
- 未選択 change を `[ ]` でマーク（将来の拡張性のため）
- 各 change の完全なファイルパス（`openspec/changes/{change_id}/proposal.md`）
- 「選択済み change のみ分析する」という明示的な指示
- 依存関係分析結果を `order`（依存関係を満たした上での推奨実行順序）と `dependencies` の両方で返すためのレスポンス指示

#### Scenario: `order` の実行順序が並列実行で直接使われる
- **GIVEN** 依存関係分析の結果として `order` が返される
- **WHEN** 並列実行が次の起動候補を評価する
- **THEN** `order` は group 変換を経ずに実行候補の順位付けとして扱われる
