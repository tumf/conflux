## Implementation Tasks

- [ ] 1. change ごとの in-flight 実行ハンドル管理を追加し、change ID から force-kill できる共通 backend 経路を実装する（verification: integration - `src/parallel/mod.rs`, `src/serial_run_service.rs`, `src/process_manager.rs` を確認し、running change の kill が cooperative flag だけでなく実プロセス終了に接続されていること）

- [ ] 2. TUI に active change 用の `K -> y` 二段階確認 force-kill 操作を追加し、`Space` は queue/unqueue のまま維持したうえで、成功時のみ対象 change を `not queued` / `selected=false` に戻す（verification: unit - `src/tui/key_handlers.rs`, `src/tui/render.rs`, `src/tui/command_handlers.rs`, `src/tui/orchestrator.rs`, `src/tui/state.rs` の確認モード、停止完了/失敗分岐、キーヒントを確認し、他 change を止めずに対象だけ遷移すること）

- [ ] 3. Web API の single-change stop-and-dequeue を running change に対する強制 kill 契約へ更新し、失敗時レスポンスを明確化する（verification: integration - `src/web/api.rs`, `src/web/mod.rs`, `src/remote/client.rs` と API テストで `POST /api/v1/projects/{project_id}/changes/{change_id}/stop-and-dequeue` の成功/失敗を確認する）

- [ ] 4. WebUI の change 行 stop 操作に確認ダイアログを追加し、confirm 後のみ backend force-kill 経路を呼ぶよう文言とエラーハンドリングを更新する（verification: unit - `dashboard/src/components/ChangeRow.tsx`, 関連ダイアログ component, `dashboard/src/api/restClient.ts`, `dashboard/src/api/restClient.test.ts` を確認し、active change で確認後にのみ force stop 呼び出しが行われること）

- [ ] 5. serial / parallel の回帰テストを追加し、active change 強制停止後も他 queued change が継続することを検証する（verification: integration - Rust テストで running change force-kill、queued 継続、kill failure の各ケースを追加して確認する）

- [ ] 6. strict validation を通す（verification: manual - `python3 "$HOME/.agents/skills/cflx-proposal/scripts/cflx.py" validate add-change-force-kill --strict`）

## Future Work

- 実行中 row に「stopping…」などの中間表示を追加するかは、force-kill の実動作が安定してから別 change で検討する
