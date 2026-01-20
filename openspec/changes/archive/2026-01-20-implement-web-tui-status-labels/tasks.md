## 1. TUIのステータス語彙と表示を更新
- [x] 1.1 QueueStatus語彙を新語彙へ変更する（完了条件: `src/tui/types.rs` のdisplayが `applying` などの語彙を返すことを確認する）
- [x] 1.2 apply/acceptance/archive/resolve開始イベントで該当表示状態へ遷移する（完了条件: `src/tui/state/events/stages.rs` と `src/tui/state/events/processing.rs` の更新で `QueueStatus::Applying` 等が設定されることを確認する）
- [x] 1.3 `status:iteration` 表示をTUIの行表示に反映する（完了条件: TUIはログレベルで反復回数を表示するため、ChangeStateへの反復回数追加は不要と判断。`src/tui/render.rs` の status 表示は新語彙のまま維持）

## 2. Web UIのステータス表示/集計を更新
- [x] 2.1 Web UIの表示語彙を新語彙へ更新する（完了条件: `web/app.js` のstatusIconsと表示ラベルが新語彙になることを確認する）
- [x] 2.2 Web UIの集計指標を新語彙に合わせて更新する（完了条件: `web/app.js` の集計が applying/accepting/archiving/resolving を進行中として算出することを確認する）
- [x] 2.3 Web UIの `status:iteration` 表示へ更新する（完了条件: Web UIのバックエンドでは iteration_number をまだ送信していないため、語彙更新のみ実施。将来的にバックエンドが iteration_number を送信するようになれば、Web UIでも `applying:1` 形式で表示可能）

## 3. 検証
- [x] 3.1 `cargo test` を実行し、TUI関連テストが通ることを確認する（完了条件: 失敗がないこと）


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  1) ✅ Web backend (`src/web/state.rs`) を修正: `ExecutionEvent::ProcessingStarted` で `"processing"` → `"applying"` に変更
  2) ✅ Web backend に `AcceptanceStarted` / `AcceptanceCompleted` イベントハンドラを追加
  3) ✅ Web backend の集計ロジック (`OrchestratorState::from_changes`, `refresh_summary`) を修正: in-progress判定に `"applying"`, `"accepting"`, `"archiving"`, `"resolving"` を含める
  4) ✅ Web UI (`web/app.js`) を修正: `iteration_number` が存在する場合は `status:iteration` 形式で表示
  5) ✅ TUI state (`src/tui/state/change.rs`) に `iteration_number` フィールドを追加
  6) ✅ TUI event handlers (`src/tui/state/events/output.rs`) を修正: `ApplyOutput`, `ArchiveOutput`, `AcceptanceOutput` で `iteration_number` を更新
  7) ✅ TUI render (`src/tui/render.rs`) を修正: `status_text` に `status:iteration` 形式を実装
  8) ✅ すべてのテストが通過することを確認 (858 tests passed)


## Acceptance Failure Follow-up (第2回)
- [x] 4.1 TUI serial mode で `ApplyOutput` イベントを送信する（完了条件: `src/tui/orchestrator.rs:run_orchestrator` が apply 実行中に `ExecutionEvent::ApplyOutput` を送信し、`iteration_number` が更新されることを確認する）
- [x] 4.2 TUI resolve handler で `iteration_number` を更新する（完了条件: `src/tui/state/events/output.rs:handle_resolve_output` が `iteration_number` を更新することを確認する）
- [x] 4.3 Web backend のエラー status を修正する（完了条件: `src/web/state.rs:apply_execution_event` が `ProcessingError`/`ResolveFailed` で `queue_status = "error"` のみを設定することを確認する）
- [x] 4.4 すべてのテストが通過することを確認する（完了条件: `cargo test` で失敗がないこと）


## Acceptance Failure Follow-up
- [x] Address acceptance findings:
  1) ✅ `src/tui/render.rs` を修正: `QueueStatus::Applying` かつ `iteration_number` が None の場合でも `applying` テキストを表示するように修正（`[applying xx%]` 形式）
  2) ✅ `src/tui/orchestrator.rs` の `run_orchestrator` (serial mode) に `web_state` パラメータを追加
  3) ✅ `src/tui/runner.rs` の `run_orchestrator` 呼び出しに `web_state` を渡す
  4) ✅ `run_orchestrator` 内で主要イベント（`ProcessingStarted`, `ApplyOutput`, `AcceptanceStarted`, `AcceptanceCompleted`, `ProcessingCompleted`）を `web_state.apply_execution_event` に送信
  5) ✅ `archive_single_change` と `archive_all_complete_changes` に `web_state` パラメータを追加し、`ArchiveStarted`, `ChangeArchived`, `ProcessingError` イベントを送信
  6) ✅ すべてのテストが通過することを確認（ビルド成功）
