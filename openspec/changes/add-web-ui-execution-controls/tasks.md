## 1. Server-side control API
- [ ] 1.1 Web制御用のHTTP APIハンドラとルータを追加する（完了条件: `src/web/api.rs` に start/stop/cancel-stop/force-stop/retry エンドポイントが追加されている）
- [ ] 1.2 Web制御のための共有コントロール状態を追加する（完了条件: TUI/Runが同一の制御経路を利用し、Web APIがそれを呼び出せる）
- [ ] 1.3 WebStateのapp_mode語彙を拡張し、stopping/errorを含めて配信する（完了条件: WebSocket `state_update` にモードが反映される）

## 2. Web UI
- [ ] 2.1 実行/停止コントロールバーUIを追加する（完了条件: Web UIにRun/Stop/Force Stop/Cancel Stop/Retryボタンが表示される）
- [ ] 2.2 Web UIがapp_modeに応じて操作を有効化/無効化する（完了条件: running/stopping/stopped/select/errorで期待通りの挙動になる）
- [ ] 2.3 実行/停止API呼び出しの成功/失敗をトースト通知する（完了条件: 成功時/失敗時にトーストが表示される）

## 3. Spec updates
- [ ] 3.1 Web monitoring仕様に制御APIとUI動作の要件を追加する（完了条件: `openspec/changes/add-web-ui-execution-controls/specs/web-monitoring/spec.md` にADDED/MODIFIED要件とシナリオがある）
- [ ] 3.2 CLI仕様にWeb制御有効化の制約を追加する（完了条件: `openspec/changes/add-web-ui-execution-controls/specs/cli/spec.md` に要件とシナリオがある）
- [ ] 3.3 OpenAPIドキュメントを更新する（完了条件: `docs/web-api.openapi.yaml` に制御APIが記載される）
- [ ] 3.4 `openspec/changes/add-web-ui-execution-controls/design.md` を必要に応じて追加/更新する
- [ ] 3.5 `npx @fission-ai/openspec@latest validate add-web-ui-execution-controls --strict` を実行し、エラーがないことを確認する
