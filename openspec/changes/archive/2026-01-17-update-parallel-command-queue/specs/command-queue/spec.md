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

#### Scenario: parallel の apply/archive が CommandQueue 経由で実行される

- **GIVEN** parallel 実行モードで apply/archive が実行される
- **WHEN** apply/archive コマンドが起動される
- **THEN** CommandQueue の stagger と retry が適用される
- **AND** streaming 出力のリトライ通知が既存の出力経路に送信される
