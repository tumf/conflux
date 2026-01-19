## 1. Implementation
- [x] 1.1 CLI仕様差分を作成する（Stopped時のqueue方針/再開条件を更新）
  - 検証: `openspec/changes/tui-stopped-queue-policy/specs/cli/spec.md` を確認し `npx @fission-ai/openspec@latest validate tui-stopped-queue-policy --strict` が通る
- [x] 1.2 Stopped遷移時にqueue状態をNotQueuedへ戻し、実行マークを保持する
  - 検証: `src/tui/state/events.rs` と `src/tui/runner.rs` の停止処理で queue_status の更新方針を確認
- [x] 1.3 F5再開時に実行マークの付いたchangeをqueued化して開始する
  - 検証: `src/tui/state/modes.rs` と `src/tui/runner.rs` の再開処理を確認
- [x] 1.4 Stopped表示が実行マーク/NotQueued方針と整合するように表示/文言を調整する
  - 検証: `src/tui/render.rs` の表示ロジックとヘルプ文言を確認
- [x] 1.5 TUI停止/再開のユニットテストを更新する
  - 検証: `cargo test` を実行し該当テストが通る


## Acceptance Failure Follow-up
- [x] Address acceptance findings: No findings - all tests pass and spec validation succeeds
