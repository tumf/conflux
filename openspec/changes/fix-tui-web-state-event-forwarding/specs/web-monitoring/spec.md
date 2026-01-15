# web-monitoring Specification Changes

## MODIFIED Requirements

### Requirement: WebSocket - Real-time Updates

HTTPサーバーは、serial 実行だけでなく parallel 実行（`--parallel`）時にも、WebSocket を通じてダッシュボードへ状態更新をブロードキャストしなければならない（SHALL）。

ダッシュボード互換性のため、`state_update` メッセージの `changes` は常に変更一覧の全件スナップショットでなければならない（MUST）。

**変更内容**: TUIモード（`tui --web`）での並列実行時にも、WebStateへのイベント転送を行うようにする。

#### Scenario: TUI + Web監視モードでの並列実行進捗がWebダッシュボードに反映される

- **GIVEN** ユーザーが `tui --web` でオーケストレーターを起動している
- **AND** 並列モードが有効である
- **AND** ダッシュボードが `/ws` へ WebSocket 接続済みである
- **WHEN** parallel 実行で `ProgressUpdated`（完了数/合計数）が発生する
- **THEN** サーバーは `{"type":"state_update", ...}` をブロードキャストする
- **AND** `changes` には当該 change の進捗（`completed_tasks/total_tasks/progress_percent`）が反映される
- **AND** `changes` は全件スナップショットである
- **AND** ダッシュボードのステータスバッジが `pending` から `in_progress`/`complete` に更新される

#### Scenario: CLIモードでの並列実行進捗がWebダッシュボードに反映される（既存動作の維持）

- **GIVEN** ユーザーが `run --web --parallel` でオーケストレーターを起動している
- **AND** ダッシュボードが `/ws` へ WebSocket 接続済みである
- **WHEN** parallel 実行で `ProgressUpdated`（完了数/合計数）が発生する
- **THEN** サーバーは `{"type":"state_update", ...}` をブロードキャストする
- **AND** `changes` には当該 change の進捗が反映される
- **AND** ダッシュボードのステータスが更新される

#### Scenario: TUIモードとCLIモードで一貫したWebSocket更新動作

- **GIVEN** TUIモード（`tui --web`）またはCLIモード（`run --web`）で実行している
- **WHEN** 並列実行でイベントが発生する
- **THEN** どちらのモードでも同じ形式の `state_update` メッセージがブロードキャストされる
- **AND** WebStateへのイベント適用ロジックは共通である
