## 1. VCS 抽象化レイヤーの実装

- [x] 1.1 `WorkspaceManager` trait に不足メソッドを追加
- [x] 1.2 `JjWorkspaceManager` に新メソッドを実装
- [x] 1.3 `GitWorkspaceManager` に新メソッドを実装

## 2. ParallelExecutor の VCS-agnostic 化

- [x] 2.1 `workspace_manager` を `Box<dyn WorkspaceManager>` に変更
- [x] 2.2 jj コマンドのハードコードを VCS 分岐に置換
- [x] 2.3 `WorkspaceCleanupGuard` を VCS-agnostic に修正

## 3. ビルド・テスト

- [x] 3.1 警告修正（未使用コード整理）
- [x] 3.2 `cargo clippy` パス
- [x] 3.3 `cargo test` パス

## 4. 動作確認

- [x] 4.1 jj リポジトリでの parallel 実行確認（既存動作）
- [x] 4.2 Git リポジトリでの parallel 実行確認
