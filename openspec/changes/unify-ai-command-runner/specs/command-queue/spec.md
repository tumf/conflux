## MODIFIED Requirements

### Requirement: すべてのコマンド種別への適用

コマンドキューはすべての `*_command` 実行に適用されなければならない (MUST)。

対象となるコマンドは以下の通り：
- `apply_command` - 変更適用
- `archive_command` - 変更アーカイブ
- `resolve_command` - 競合解消・マージ完了
- `analyze_command` - 依存関係分析
- `worktree_command` - ワークツリー上の提案作成

**実装要件**:
- すべてのAI駆動コマンドは共通ランナー層（`AiCommandRunner`）を経由しなければならない (MUST)
- 時間差起動の状態（`last_execution`）はプロセス全体で共有されなければならない (MUST)
- 並列実行モードの apply/archive も共通ランナー層を経由しなければならない (MUST)
- resolve 実行時に `AgentRunner` を都度作成してはならない (MUST NOT)

#### Scenario: apply_command での時間差起動とリトライ

- **WHEN** `apply_command` を実行
- **THEN** 時間差起動メカニズムが適用される
- **AND** リトライ可能エラー時に自動リトライが実行される

#### Scenario: resolve_command での優先的リトライ

- **WHEN** `resolve_command` を実行
- **THEN** 時間差起動メカニズムが適用される
- **AND** 競合解消やマージ操作の一時的エラーで自動リトライが実行される

#### Scenario: すべてのコマンドで統一された動作

- **GIVEN** すべての `*_command` が設定されている
- **WHEN** 各コマンドを順次実行
- **THEN** すべてのコマンドで同じキュー設定（遅延時間、リトライ）が適用される
- **AND** コマンド種別による動作の違いがない

#### Scenario: 並列 apply/archive での stagger 適用

- **GIVEN** 並列実行モードで複数の change が処理されている
- **AND** 遅延時間が2秒に設定されている
- **WHEN** worktree A で apply コマンドが実行される
- **AND** 0.5秒後に worktree B で apply コマンドが実行されようとする
- **THEN** worktree B の apply は1.5秒待機してから実行される
- **AND** 両方の apply が共通の `last_execution` 状態を参照している

#### Scenario: resolve での stagger 状態共有

- **GIVEN** 並列実行モードで resolve が必要になった
- **AND** 直前に apply コマンドが実行された
- **WHEN** resolve コマンドが実行されようとする
- **THEN** resolve は apply と同じ `last_execution` 状態を参照する
- **AND** 遅延時間内であれば待機してから実行される

## ADDED Requirements

### Requirement: analyze 出力の厳格検証

`analyze_command` の出力は、exit code が 0 であっても JSON として有効かつ期待スキーマに準拠しなければエラーとしなければならない (MUST)。

期待スキーマは以下の通り：
```json
{
  "groups": [
    { "id": 1, "changes": ["change-a"], "depends_on": [] }
  ]
}
```

検証項目：
1. stdout が JSON としてパース可能であること
2. トップレベルに `groups` キーが存在すること
3. `groups` が配列であること

#### Scenario: exit 0 でも JSON が壊れていたらエラー

- **GIVEN** `analyze_command` が exit code 0 で終了した
- **AND** stdout が有効な JSON ではない（例: 途中で切れた、構文エラー）
- **WHEN** 出力検証が実行される
- **THEN** エラーが返される
- **AND** エラーメッセージに「JSON parse failed」が含まれる
- **AND** stdout の先頭部分がエラーメッセージに含まれる（デバッグ用）

#### Scenario: groups キーが存在しない場合

- **GIVEN** `analyze_command` が exit code 0 で終了した
- **AND** stdout は有効な JSON だが `groups` キーがない（例: `{"result": "ok"}`）
- **WHEN** 出力検証が実行される
- **THEN** エラーが返される
- **AND** エラーメッセージに「missing required key: groups」が含まれる

#### Scenario: 正常な JSON で検証成功

- **GIVEN** `analyze_command` が exit code 0 で終了した
- **AND** stdout が期待スキーマに準拠した JSON である
- **WHEN** 出力検証が実行される
- **THEN** 検証が成功し、パース済みの `AnalysisResult` が返される
