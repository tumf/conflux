## 1. 仕様整理
- [x] 1.1 configuration のデフォルトworktreeディレクトリ仕様を更新する
- [x] 1.2 parallel-execution のデフォルト解決シナリオを更新する
- [x] 1.3 tui-worktree-view のデフォルト解決シナリオを更新する

## 2. 実装
- [x] 2.1 workspace_base_dir の既定値解決ロジックを追加する
- [x] 2.2 project_slug の生成ルールを実装する
- [x] 2.3 並列実行とTUIのworktree作成で共通ロジックを使用する
- [x] 2.4 旧一時ディレクトリ利用のフォールバック条件を実装する

## 3. 検証
- [x] 3.1 workspace_base_dir 未設定時のパス解決テストを追加/更新する
- [x] 3.2 既存の関連テストが通ることを確認する
