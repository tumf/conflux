## 1. 準備

- [ ] 1.1 `src/vcs/` ディレクトリ構造を作成
- [ ] 1.2 `VcsError` 型を `src/vcs/mod.rs` に定義

## 2. 共通コマンドヘルパーの抽出

- [ ] 2.1 `run_vcs_command()` を `src/vcs/commands.rs` に実装
- [ ] 2.2 出力パース用のユーティリティ関数を追加

## 3. Jujutsu 実装の移行

- [ ] 3.1 `jj_commands.rs` → `src/vcs/jj/commands.rs` に移動
- [ ] 3.2 `jj_workspace.rs` → `src/vcs/jj/mod.rs` に移動
- [ ] 3.3 共通ヘルパーを使用するようにリファクタリング

## 4. Git 実装の移行

- [ ] 4.1 `git_commands.rs` → `src/vcs/git/commands.rs` に移動
- [ ] 4.2 `git_workspace.rs` → `src/vcs/git/mod.rs` に移動
- [ ] 4.3 共通ヘルパーを使用するようにリファクタリング

## 5. トレイトと公開 API の整理

- [ ] 5.1 `vcs_backend.rs` の内容を `src/vcs/mod.rs` に統合
- [ ] 5.2 `WorkspaceManager` トレイトのメソッドシグネチャを見直し
- [ ] 5.3 古いファイルから re-export を設定（後方互換）

## 6. エラー型の統合

- [ ] 6.1 `OrchestratorError` から VCS 関連 variant を `VcsError` に移行
- [ ] 6.2 `From<VcsError> for OrchestratorError` を実装

## 7. テストと検証

- [ ] 7.1 既存テストが通ることを確認 (`cargo test`)
- [ ] 7.2 clippy 警告がないことを確認
- [ ] 7.3 E2E テストを実行
