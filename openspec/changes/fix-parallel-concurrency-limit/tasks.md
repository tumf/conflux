## 1. Implementation
- [ ] 1.1 並列実行の同時数上限をworktree作成フェーズにも適用する
- [ ] 1.2 execute_group内のworktree一括作成を、semaphore制御下で作成する方式に変更する
- [ ] 1.3 TUI/CLIの並列実行フローで上限が反映されることを確認する
- [ ] 1.4 既存の動的キュー/再分析の挙動が上限適用後も破綻しないことを確認する

## 2. Validation
- [ ] 2.1 変更後に並列実行で同時worktree数が設定値を超えないことを検証する
- [ ] 2.2 必要に応じて `cargo test` を実行する
