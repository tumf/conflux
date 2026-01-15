## MODIFIED Requirements

### Requirement: WebSocket - Real-time Updates
HTTPサーバーは、serial 実行だけでなく parallel 実行（`--parallel`）時にも、WebSocket を通じてダッシュボードへ状態更新をブロードキャストしなければならない（SHALL）。

ダッシュボード互換性のため、`state_update` メッセージの `changes` は常に変更一覧の全件スナップショットでなければならない（MUST）。

#### Scenario: parallel 実行の進捗が Web ダッシュボードに反映される
- **GIVEN** ユーザーが `--web --parallel` でオーケストレーターを起動している
- **AND** ダッシュボードが `/ws` へ WebSocket 接続済みである
- **WHEN** parallel 実行で `ProgressUpdated`（完了数/合計数）が発生する
- **THEN** サーバーは `{"type":"state_update", ...}` をブロードキャストする
- **AND** `changes` には当該 change の進捗（`completed_tasks/total_tasks/progress_percent`）が反映される
- **AND** `changes` は全件スナップショットである
- **AND** ダッシュボードのステータスバッジが `pending` から `in_progress`/`complete` に更新される
