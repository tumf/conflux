## Implementation Tasks

- [x] 1. change ごとの in-flight 実行ハンドル管理を追加し、change ID から force-kill できる共通 backend 経路を実装する（verification: integration - `src/tui/queue.rs` に `KillRegistry` を追加し、`register_kill_token` / `unregister_kill_token` / `force_kill` メソッドを実装。`src/parallel/dispatch.rs` で workspace task 作成時にトークンを登録、`src/parallel/queue_state.rs` で完了時にクリーンアップ。`src/tui/command_handlers.rs` の `DequeueChange` ハンドラが `force_kill()` を呼び出してトークンを即時キャンセルすることを確認）

- [x] 2. TUI に active change 用の `K -> y` 二段階確認 force-kill 操作を追加し、`Space` は queue/unqueue のまま維持したうえで、成功時のみ対象 change を `not queued` / `selected=false` に戻す（verification: unit - `src/tui/types.rs` に `ConfirmForceKill` モードを追加、`src/tui/key_handlers.rs` で `K` キーが確認モード開始、`y` で確認、`n`/`Esc` でキャンセル。`src/tui/state.rs` で active change への Space が `ToggleActionResult::None` を返すこと。`src/tui/render.rs` でキーヒントが `K: kill` / `Y: confirm kill` / `N: cancel` を表示。テスト `test_running_mode_space_on_active_change_does_not_stop` と `test_running_mode_space_on_accepting_does_not_stop` で検証）

- [x] 3. Web API の single-change stop-and-dequeue を running change に対する強制 kill 契約へ更新し、失敗時レスポンスを明確化する（verification: integration - `src/server/api/control.rs` に `stop_and_dequeue_change` ハンドラを追加、`src/server/api/mod.rs` にルート登録。active change の場合はプロジェクトランナーを停止して force-kill を保証。テスト `test_stop_and_dequeue_change_deselects_and_returns_ok` と `test_stop_and_dequeue_change_not_found_project` で検証）

- [x] 4. WebUI の change 行 stop 操作に確認ダイアログを追加し、confirm 後のみ backend force-kill 経路を呼ぶよう文言とエラーハンドリングを更新する（verification: unit - `dashboard/src/components/StopChangeDialog.tsx` を新規作成、`dashboard/src/components/ChangeRow.tsx` で Stop ボタンクリック時にダイアログを表示し、confirm 後のみ API 呼び出し。`dashboard/src/components/ChangeRow.test.tsx` で「クリックでダイアログ表示」「confirm で API 呼び出し」「cancel で API 非呼び出し」を検証）

- [x] 5. serial / parallel の回帰テストを追加し、active change 強制停止後も他 queued change が継続することを検証する（verification: integration - `src/tui/queue.rs` に `test_force_kill_marks_stopped_and_cancels_token`、`test_force_kill_without_token_still_marks_stopped`、`test_unregister_kill_token` を追加。`src/tui/state.rs` に Space on active change のテストを追加。`src/server/api/control.rs` に stop-and-dequeue API テストを追加。全 1430 テスト通過）

- [x] 6. strict validation を通す（verification: manual - `python3 "$HOME/.claude/skills/cflx-workflow/scripts/cflx.py" validate add-change-force-kill --strict` で通過、`cargo clippy --all-targets` 警告ゼロ）

## Future Work

- 実行中 row に「stopping…」などの中間表示を追加するかは、force-kill の実動作が安定してから別 change で検討する
