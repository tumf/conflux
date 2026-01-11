# Tasks: Git Worktree による並列実行モード

## 1. 基盤実装

- [x] 1.1 `src/error.rs` に Git 用エラー型を追加
  - `GitCommand(String)`: Git コマンド実行エラー
  - `GitConflict(String)`: Git マージコンフリクト
  - `GitUncommittedChanges(String)`: 未コミット変更エラー
  - `NoVcsBackend`: VCS バックエンド未検出エラー

- [x] 1.2 `src/git_commands.rs` を作成（Git コマンドヘルパー）
  - `run_git()`: Git コマンド実行
  - `check_git_repo()`: Git リポジトリ判定
  - `has_uncommitted_changes()`: 未コミット/未追跡チェック
  - `get_current_commit()`: 現在の HEAD 取得
  - `get_current_branch()`: 現在のブランチ名取得
  - `get_status()`: git status 出力取得
  - `worktree_add()`: worktree 作成
  - `worktree_remove()`: worktree 削除
  - `branch_delete()`: ブランチ削除
  - `checkout()`: ブランチチェックアウト
  - `merge()`: マージ実行

- [x] 1.3 `src/vcs_backend.rs` を作成（VCS バックエンド trait）
  - `VcsBackend` enum: `Auto`, `Jj`, `Git`
  - `WorkspaceManager` trait 定義
  - `WorkspaceStatus` enum 定義
  - `Workspace` 構造体定義
  - `WorkspaceResult` 構造体定義
  - `detect_vcs_backend()`: 自動検出関数

## 2. JjWorkspaceManager のリファクタリング

- [x] 2.1 `JjWorkspaceManager` を `WorkspaceManager` trait 実装に変換
  - 既存の公開メソッドを trait メソッドにマッピング
  - 内部実装は変更なし
  - `WorkspaceStatus` を `vcs_backend` モジュールから再エクスポート

## 3. GitWorkspaceManager 実装

- [x] 3.1 `src/git_workspace.rs` を作成
  - `GitWorkspace` 構造体
  - `GitWorkspaceManager` 構造体
  - `WorkspaceManager` trait 実装

- [x] 3.2 ワークスペース作成機能
  - `check_available()`: Git リポジトリ判定
  - `check_clean_working_directory()`: 未コミット変更チェック（エラー時は詳細メッセージ）
  - `create_workspace()`: `git worktree add` でワークスペース作成
  - `get_current_revision()`: 現在のコミットハッシュ取得

- [x] 3.3 マージ機能（逐次マージ）
  - `merge_workspaces()`: 複数ブランチを1つずつマージ
  - 各マージでコンフリクト検出
  - コンフリクト時は `GitConflict` エラーを返す

- [x] 3.4 クリーンアップ機能
  - `cleanup_workspace()`: `git worktree remove` + `git branch -D`
  - `cleanup_all()`: 全ワークスペースのクリーンアップ

- [x] 3.5 コンフリクト解決サポート
  - `conflict_resolution_prompt()`: Git コンフリクトマーカー情報を含むプロンプト

## 4. ParallelExecutor の VCS 抽象化

- [x] 4.1 `ParallelExecutor` が `vcs_backend::WorkspaceStatus` を使用するよう更新

- [x] 4.2 VCS 選択のための設定・CLI オプション追加（5.x で実装）

## 5. 設定とCLI

- [x] 5.1 `src/config.rs` に VCS 設定オプション追加
  - `vcs_backend: Option<VcsBackend>` フィールド追加
  - `get_vcs_backend()` メソッド

- [x] 5.2 `src/cli.rs` に `--vcs` オプション追加
  - `--vcs auto|jj|git` オプション
  - デフォルト: `auto`（自動検出）

- [x] 5.3 `src/orchestrator.rs` に VCS 設定を追加
  - `vcs_backend` フィールド追加
  - CLI/config から VCS 設定を取得

## 6. TUI 対応

- [x] 6.1 Git エラー型は TUI でも適切に表示される（既存のエラーハンドリングで対応）

- [x] 6.2 VCS タイプの表示は将来の拡張として保留

## 7. テスト

- [x] 7.1 `git_commands.rs` のユニットテスト
  - コマンド実行ヘルパーのテスト
  - 未コミット変更検出のテスト

- [x] 7.2 `git_workspace.rs` のユニットテスト
  - マネージャ作成のテスト
  - ワークスペース名サニタイズのテスト

- [x] 7.3 VCS 自動検出のテスト
  - `vcs_backend.rs` に VcsBackend パース/表示のテスト

- [x] 7.4 E2E テスト
  - Git リポジトリでの並列実行テスト
  - 未コミット変更時のエラー確認テスト
  - jj 既存テストが引き続きパスすることの確認

## 8. ドキュメント

- [x] 8.1 README に Git 並列実行の説明を追加
  - 前提条件（クリーンな作業ディレクトリ）
  - `--vcs` オプションの説明
  - VCS バックエンド選択の説明
  - 使用例
