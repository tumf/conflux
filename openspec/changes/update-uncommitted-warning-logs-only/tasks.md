## 1. 仕様更新
- [ ] 1.1 `openspec/changes/update-uncommitted-warning-logs-only/specs/parallel-execution/spec.md` に TUI の未コミット警告をログのみとする要件を追記する

## 2. 実装
- [ ] 2.1 `src/tui/state/events.rs` で未コミット警告イベントはポップアップを出さずログのみ記録する
- [ ] 2.2 `src/tui/state/mod.rs` の警告ポップアップ関連テストを更新する

## 3. 検証
- [ ] 3.1 `cargo test` を実行する
