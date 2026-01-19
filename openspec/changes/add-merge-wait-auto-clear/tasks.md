## 1. 仕様と状態遷移の更新
- [ ] 1.1 `QueueStatus::MergeWait` の自動解除条件と遷移先を spec に追記する（解除後は Queued と明記する）
  - 検証: `openspec/changes/add-merge-wait-auto-clear/specs/tui-architecture/spec.md` に MODIFIED 要件と Scenario が追加されている

## 2. TUI 自動更新の判定実装
- [ ] 2.1 5秒ポーリング時に worktree の有無と ahead 状態を評価し、MergeWait を Queued に戻す処理を追加する
  - 検証: `src/tui/state/events.rs` の自動更新処理に MergeWait 解除ロジックが追加されている

## 3. 表示と操作ヒントの整合
- [ ] 3.1 MergeWait が解除された場合に `M` ヒントが出ないことを保証する
  - 検証: `src/tui/render.rs` で MergeWait のみが `M` を表示していることを確認できる

## 4. テストと検証
- [ ] 4.1 MergeWait の自動解除を検証するテストを追加する
  - 検証: `cargo test` 実行で追加テストが通る
- [ ] 4.2 既存の MergeWait 状態維持テストを更新する
  - 検証: `cargo test` が失敗しない
