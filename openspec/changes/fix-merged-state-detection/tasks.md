## 1. State Detection Logic
- [x] 1.1 `WorkspaceState::Merged` 判定に changes ディレクトリ消失チェックを追加する
- [x] 1.2 `Archive:` コミットがあるのに changes が残る場合は warn ログを出す
- [x] 1.3 既存の archived/applied 判定に影響しないことを確認する

## 2. Parallel/TUI Event Handling
- [x] 2.1 Merged 判定でスキップする場合に `MergeCompleted` を送出する
- [x] 2.2 TUI が `Merged` に遷移し 0% 停止が起きないことを確認する

## 3. Validation
- [x] 3.1 `npx @fission-ai/openspec@latest validate fix-merged-state-detection --strict` を実行する
