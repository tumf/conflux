# Tasks: Git Worktree による並列実行モード

## 1. 基盤実装

- [ ] 1.1 `src/error.rs` に Git 用エラー型を追加
  - `GitCommand(String)`: Git コマンド実行エラー
  - `GitConflict(String)`: Git マージコンフリクト
  - `GitUncommittedChanges(String)`: 未コミット変更エラー

- [ ] 1.2 `src/git_commands.rs` を作成（Git コマンドヘルパー）
  - `run_git()`: Git コマンド実行
  - `check_git_repo()`: Git リポジトリ判定
  - `has_uncommitted_changes()`: 未コミット/未追跡チェック
  - `get_current_commit()`: 現在の HEAD 取得
  - `get_status()`: git status 出力取得

- [ ] 1.3 `src/vcs_backend.rs` を作成（VCS バックエンド trait）
  - `VcsBackend` enum: `Jj`, `Git`
  - `WorkspaceManager` trait 定義
  - `detect_vcs_backend()`: 自動検出関数

## 2. JjWorkspaceManager のリファクタリング

- [ ] 2.1 `JjWorkspaceManager` を `WorkspaceManager` trait 実装に変換
  - 既存の公開メソッドを trait メソッドにマッピング
  - 内部実装は変更なし
  - 既存テストが全てパスすることを確認

## 3. GitWorkspaceManager 実装

- [ ] 3.1 `src/git_workspace.rs` を作成
  - `GitWorkspace` 構造体
  - `GitWorkspaceManager` 構造体
  - `WorkspaceManager` trait 実装

- [ ] 3.2 ワークスペース作成機能
  - `check_available()`: Git リポジトリ判定
  - `check_clean_working_directory()`: 未コミット変更チェック（エラー時は詳細メッセージ）
  - `create_workspace()`: `git worktree add` でワークスペース作成
  - `get_current_revision()`: 現在のコミットハッシュ取得

- [ ] 3.3 マージ機能（逐次マージ）
  - `merge_workspaces()`: 複数ブランチを1つずつマージ
  - 各マージでコンフリクト検出
  - コンフリクト時は `GitConflict` エラーを返す

- [ ] 3.4 クリーンアップ機能
  - `cleanup_workspace()`: `git worktree remove` + `git branch -D`
  - `cleanup_all()`: 全ワークスペースのクリーンアップ

- [ ] 3.5 コンフリクト解決サポート
  - `detect_conflicts()`: `git diff --name-only --diff-filter=U` でコンフリクトファイル検出
  - AgentRunner 用のプロンプトに Git コンフリクトマーカー情報を含める

## 4. ParallelExecutor の VCS 抽象化

- [ ] 4.1 `ParallelExecutor` を VCS バックエンド抽象化に対応
  - コンストラクタで `Box<dyn WorkspaceManager>` を受け取る
  - VCS 固有の操作を trait メソッド経由で呼び出し
  - コンフリクト解決プロンプトを VCS タイプに応じて調整

- [ ] 4.2 VCS バックエンドファクトリ関数
  - `create_workspace_manager()`: 検出結果に基づいて適切なマネージャを生成

## 5. 設定とCLI

- [ ] 5.1 `src/config.rs` に VCS 設定オプション追加
  - `vcs_backend: Option<VcsBackend>` フィールド追加
  - `VcsBackend` enum: `Auto`, `Jj`, `Git`
  - `get_vcs_backend()` メソッド

- [ ] 5.2 `src/cli.rs` に `--vcs` オプション追加
  - `--vcs auto|jj|git` オプション
  - デフォルト: `auto`（自動検出）

- [ ] 5.3 ParallelRunService での VCS 選択ロジック
  - CLI/config から VCS 設定を取得
  - 自動検出または指定された VCS を使用
  - 利用不可の場合は適切なエラー

## 6. TUI 対応

- [ ] 6.1 Git 未コミット変更エラーのポップアップ表示
  - F5 押下時に未コミット変更があればポップアップ
  - エラーメッセージと解決手順を表示
  - Enter キーで閉じる

- [ ] 6.2 VCS タイプの表示（オプション）
  - 並列実行時にヘッダーまたはログで VCS タイプを表示

## 7. テスト

- [ ] 7.1 `git_commands.rs` のユニットテスト
  - コマンド実行ヘルパーのテスト
  - 未コミット変更検出のテスト

- [ ] 7.2 `git_workspace.rs` のユニットテスト
  - ワークスペース作成/削除のテスト
  - マージ処理のテスト
  - コンフリクト検出のテスト

- [ ] 7.3 VCS 自動検出のテスト
  - jj のみ存在 → jj 選択
  - git のみ存在 → git 選択
  - 両方存在 → jj 優先
  - 両方なし → エラー

- [ ] 7.4 E2E テスト
  - Git リポジトリでの並列実行テスト
  - 未コミット変更時のエラー確認テスト
  - jj 既存テストが引き続きパスすることの確認

## 8. ドキュメント

- [ ] 8.1 README に Git 並列実行の説明を追加
  - 前提条件（クリーンな作業ディレクトリ）
  - `--vcs` オプションの説明
  - トラブルシューティング
