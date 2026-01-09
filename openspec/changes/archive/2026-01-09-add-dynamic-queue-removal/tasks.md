## 1. 実装

- [x] 1.1 `toggle_selection` メソッドを修正し、`QueueStatus::Queued` 状態の変更を `NotQueued` に戻せるようにする
- [x] 1.2 `TuiCommand::RemoveFromQueue(String)` コマンドを追加
- [x] 1.3 `run_tui_loop` 関数でキュー解除を処理するロジックを追加
- [x] 1.4 UIのヘルプテキストを更新（"Space: toggle queue"）

## 2. テスト

- [x] 2.1 `toggle_selection` のキュー解除テストを追加
- [x] 2.2 Processing状態の変更が解除できないことを確認するテストを追加

## 3. ドキュメント

- [ ] 3.1 CLIスペックの更新（変更提案適用後に自動反映）
