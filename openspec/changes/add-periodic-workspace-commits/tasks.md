# Implementation Tasks

## 1. コアロジック実装

- [x] 1.1 `src/parallel/executor.rs` に `create_progress_commit` 関数を追加
- [x] 1.2 jjバックエンド用のコミット処理を実装（`jj describe`）
- [x] 1.3 Gitバックエンド用のコミット処理を実装（`git add -A && git commit --amend`）
- [x] 1.4 applyループ内で進捗があった場合にコミットを呼び出す処理を追加
- [x] 1.5 コミットメッセージ形式を `WIP: {change_id} ({completed}/{total} tasks)` で実装

## 2. テスト

- [x] 2.1 `create_progress_commit` 関数のユニットテストを追加
- [x] 2.2 jjバックエンドでのコミット動作テスト
- [x] 2.3 Gitバックエンドでのコミット動作テスト
- [x] 2.4 進捗がない場合にコミットが作成されないことを確認するテスト

## 3. 検証

- [x] 3.1 `cargo fmt` と `cargo clippy` を実行
- [x] 3.2 `cargo test` で全テストがパスすることを確認
- [x] 3.3 実際の並列実行でコミットが作成されることを手動確認
