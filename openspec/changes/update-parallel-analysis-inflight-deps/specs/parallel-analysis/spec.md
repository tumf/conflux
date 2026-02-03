## MODIFIED Requirements
### Requirement: Parallel dependency analysis prompt

依存分析プロンプトは、選択された変更（`is_approved = true`）を明示的に示し、各変更の提案ファイルパスを含めなければならない（SHALL）。

プロンプトは以下を含めなければならない（MUST）：
- 選択された変更を `[x]` でマークする
- 未選択の変更を `[ ]` でマークする（将来拡張のため）
- 各変更の完全なファイルパス（`openspec/changes/{change_id}/proposal.md`）
- 選択された変更のみを分析対象とする明示的な指示
- `order`（依存関係を尊重した推奨実行順）と `dependencies` を返す指示
- `dependencies` は他の変更の成果物・仕様・API に依存し、それなしに成立しない場合にのみ付与するという説明
- `order` は優先度や効率のための推奨順序であり依存関係とは独立であるという説明

加えて、プロンプトは実行中（in-flight）の変更一覧を含め、これらは選択対象ではなく依存関係判定の対象としてのみ扱うよう明示しなければならない（MUST）。

プロンプト構築と出力解析は別関数に分割してもよい（MAY）。ただし、プロンプト内容と選別ルールは既存と同一でなければならない（MUST）。

#### Scenario: Return dependencies only for mandatory conditions
- **GIVEN** 複数の変更が依存分析に含まれている
- **AND** ある変更が別の変更の成果物・仕様・API を必須条件として必要としている
- **WHEN** 依存分析が実行される
- **THEN** `dependencies` には必須の関係のみが含まれる
- **AND** 優先度や順序の好みによる関係は `dependencies` から除外される

#### Scenario: Include in-flight changes for dependency analysis
- **GIVEN** 変更 A が実行中（in-flight）であり、変更 B が A の成果物に依存する
- **WHEN** Dynamic キューへの追加により依存分析が再実行される
- **THEN** 分析プロンプトには実行中の変更一覧に A が含まれる
- **AND** `dependencies` には B → A の必須依存が含まれる
