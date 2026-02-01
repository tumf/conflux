## 1. 実装
- [ ] 1.1 runner 由来の worktree/terminal ヘルパーを専用モジュールに移す（検証: src/tui/runner.rs のヘルパーが新モジュールに移動していることを確認）
- [ ] 1.2 command_handlers/key_handlers が新モジュール経由でヘルパーを参照する（検証: runner への直接参照が消えていることを確認）
- [ ] 1.3 既存の公開 API と挙動を維持する（検証: 既存の公開関数・型のエクスポートが保持されていることを確認）
- [ ] 1.4 既存の挙動維持を確認するため `cargo test` を実行する（検証: `cargo test` が成功）
