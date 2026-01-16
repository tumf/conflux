# タスクリスト

## Phase 1: 基本構造とWorktreeビュー (8-10時間)

### 1.1 型定義の追加
- [ ] `src/tui/types.rs`: `ViewMode` enum追加 (Changes/Worktrees)
- [ ] `src/tui/types.rs`: `WorktreeInfo` struct追加
  - `path`, `head`, `branch`, `is_detached`, `is_main`, `merge_conflict`
  - `display_label()`, `display_branch()`, `has_merge_conflict()`, `conflict_file_count()` メソッド
- [ ] `src/tui/types.rs`: `WorktreeAction` enum追加 (Delete/OpenEditor/OpenShell)
- [ ] `src/tui/types.rs`: `MergeConflictInfo` struct追加
- [ ] 単体テスト: 各型のメソッドとロジックをテスト

**検証**: `cargo test types` がパス

### 1.2 Git worktree操作の実装
- [ ] `src/vcs/git/commands.rs`: `list_worktrees()` 関数実装
  - `git worktree list --porcelain` 実行
  - Porcelain形式のパーサー実装 (worktree/HEAD/branch/detached)
  - 最初のworktreeを `is_main=true` として扱う
- [ ] `src/vcs/git/commands.rs`: `worktree_remove()` 関数実装
  - `git worktree remove <path>` 実行
- [ ] `src/vcs/git/commands.rs`: `is_working_directory_clean()` 関数実装
  - `git status --porcelain` でチェック
- [ ] `src/vcs/git/commands.rs`: `get_current_branch()` 関数実装
  - `git branch --show-current` 実行
- [ ] 統合テスト: 実際のgit repositoryを使用したテスト

**検証**: `cargo test vcs::git::commands` がパス、手動でgit worktreeコマンドの動作確認

### 1.3 AppStateの拡張
- [ ] `src/tui/state/mod.rs`: Worktree関連フィールド追加
  - `view_mode`, `worktrees`, `worktree_cursor_index`, `worktree_list_state`, `pending_worktree_action`
- [ ] `src/tui/state/mod.rs`: Worktreeナビゲーションメソッド実装
  - `worktree_cursor_up()`, `worktree_cursor_down()`
- [ ] `src/tui/state/mod.rs`: Worktree削除リクエスト実装
  - `request_worktree_delete_from_list()` - バリデーション含む
  - Main worktreeチェック、Processing中チェック
- [ ] `src/tui/state/mod.rs`: ユーティリティメソッド実装
  - `get_selected_worktree_path()`, `get_selected_worktree()`
  - `confirm_worktree_action_delete()`, `cancel_worktree_action()`
- [ ] 単体テスト: カーソル移動、削除バリデーション

**検証**: `cargo test tui::state` がパス

### 1.4 イベント定義
- [ ] `src/tui/events.rs`: `TuiCommand` に追加
  - `DeleteWorktreeByPath(PathBuf)`
- [ ] `src/tui/events.rs`: `OrchestratorEvent` に追加
  - `WorktreesRefreshed { worktrees: Vec<WorktreeInfo> }`

**検証**: コンパイルエラーがないこと

### 1.5 Worktreeビューのレンダリング
- [ ] `src/tui/render.rs`: `render()` 関数を修正
  - `view_mode` に基づいてview切り替え
- [ ] `src/tui/render.rs`: `render_worktree_view()` 実装
  - Header/List/Footer のレイアウト
- [ ] `src/tui/render.rs`: `render_worktree_list()` 実装
  - Worktree情報の表示 (label, branch)
  - カーソルとハイライト
  - 動的キーヒント (基本版: Tab, D, e, Enter, +)
- [ ] `src/tui/render.rs`: `render_footer_worktree()` 実装
  - Worktree数、警告メッセージ表示
- [ ] `src/tui/render.rs`: Changesビューのキーヒント修正
  - `D` と `+` を削除、`Tab: worktrees` を追加

**検証**: TUIを起動してWorktreeビューが表示されること (データなしでもOK)

### 1.6 キーバインドの実装
- [ ] `src/tui/runner.rs`: Tabキーでview切り替え実装
  - Changes → Worktrees (worktreeリスト読み込み)
  - Worktrees → Changes
- [ ] `src/tui/runner.rs`: Worktreeビュー用のカーソル操作
  - ↑↓/jk キーをview依存で処理
- [ ] `src/tui/runner.rs`: Dキー (Worktree削除) 実装
  - Worktreeビューでのみ動作
  - 確認ダイアログ表示
- [ ] `src/tui/runner.rs`: eキー (エディタ) を両view対応に修正
  - Changes: 既存動作維持
  - Worktrees: `launch_editor_in_dir()` 使用
- [ ] `src/tui/runner.rs`: Enterキー (シェル) 実装
  - Worktreeビューでworktree_command実行
  - 設定なしの場合は無効化
- [ ] `src/tui/runner.rs`: +キー (Worktree作成) をWorktreeビューに移動
  - ws-session-{timestamp} 命名に変更
  - ブランチ作成 (detachedではなく)
- [ ] `src/tui/runner.rs`: Changesビューから D と + キーハンドラを削除
- [ ] `src/tui/utils.rs`: `launch_editor_in_dir()` 実装
  - 指定ディレクトリでエディタを起動

**検証**: 各キーバインドが期待通り動作すること、Changesビューで D/+ が無効になること

### 1.7 コマンド処理
- [ ] `src/tui/runner.rs`: `DeleteWorktreeByPath` コマンドハンドラ実装
  - `worktree_remove()` 呼び出し
  - 成功時にworktreeリスト更新
  - エラーログ表示
- [ ] `src/tui/state/events.rs`: `WorktreesRefreshed` イベントハンドラ実装
  - worktreeリスト更新
  - カーソル範囲外チェック

**検証**: Worktree削除が動作すること、リストが更新されること

### 1.8 Phase 1 統合テスト
- [ ] E2Eテスト: Tabキーでview切り替え
- [ ] E2Eテスト: Worktree作成 → リスト表示 → 削除
- [ ] E2Eテスト: エディタ・シェル起動

**検証**: 全てのE2Eテストがパス、既存テストも全てパス

---

## Phase 2: ブランチマージ機能 (6-8時間)

### 2.1 コンフリクト検出の実装
- [ ] `src/vcs/git/commands.rs`: `check_merge_conflicts()` 関数実装
  - `git merge --no-commit --no-ff <branch>` でテストマージ
  - コンフリクト検出 (stderr解析)
  - `git merge --abort` で元に戻す
  - `parse_conflict_files()` でファイル名抽出
- [ ] 単体テスト: コンフリクトパーサーのテスト
- [ ] 統合テスト: 実際のgit repositoryでコンフリクト検出

**検証**: コンフリクトありのブランチを正しく検出できること

### 2.2 ブランチマージの実装
- [ ] `src/vcs/git/commands.rs`: `merge_branch()` 関数実装
  - ワーキングディレクトリクリーンチェック
  - `git merge --no-ff --no-edit <branch>` 実行
  - コンフリクト時に `git merge --abort`
  - エラーメッセージの解析と返却
- [ ] 統合テスト: 正常マージとコンフリクトマージ

**検証**: コンフリクトなしのマージが成功すること、コンフリクトありは中断されること

### 2.3 マージイベントの追加
- [ ] `src/tui/events.rs`: `TuiCommand::MergeWorktreeBranch` 追加
  - `worktree_path`, `branch_name` フィールド
- [ ] `src/tui/events.rs`: マージ関連イベント追加
  - `MergeStarted { branch_name }`
  - `MergeCompleted { branch_name }`
  - `MergeFailed { branch_name, error }`

**検証**: コンパイルエラーがないこと

### 2.4 マージリクエストの実装
- [ ] `src/tui/state/mod.rs`: `request_merge_worktree_branch()` 実装
  - Main worktreeチェック
  - Detached HEADチェック
  - コンフリクトありチェック
  - `TuiCommand::MergeWorktreeBranch` 生成
- [ ] 単体テスト: 各バリデーションケース

**検証**: マージ不可条件で正しく拒否されること

### 2.5 マージイベントハンドリング
- [ ] `src/tui/state/events.rs`: マージイベントハンドラ実装
  - `MergeStarted`: ログ表示
  - `MergeCompleted`: 成功ログ表示
  - `MergeFailed`: エラーログとポップアップ表示

**検証**: イベント受信時に適切なログ・ポップアップが表示されること

### 2.6 マージキーバインド
- [ ] `src/tui/runner.rs`: Mキー (Shift+M) ハンドラ実装
  - Worktreeビューでのみ動作
  - `request_merge_worktree_branch()` 呼び出し
- [ ] `src/tui/runner.rs`: `MergeWorktreeBranch` コマンドハンドラ実装
  - 非同期タスクでマージ実行
  - `get_current_branch()` で base branch 取得
  - `merge_branch()` 実行
  - イベント送信 (Started/Completed/Failed)

**検証**: Mキーでマージが実行されること、ログが表示されること

### 2.7 Phase 2 統合テスト
- [ ] E2Eテスト: コンフリクトなしマージ
- [ ] E2Eテスト: コンフリクトあり時にマージ不可
- [ ] E2Eテスト: エラー時のポップアップ表示

**検証**: マージ機能が正常動作すること

---

## Phase 3: コンフリクト事前検出と最適化 (4-6時間)

### 3.1 Worktreeリスト取得の拡張
- [ ] `src/tui/runner.rs`: `load_worktrees_with_conflict_check()` 実装
  - `list_worktrees()` 実行
  - 各worktreeについてコンフリクトチェック
  - Main/Detached はスキップ
  - チェック失敗時は `merge_conflict = None`
- [ ] Tabキー切り替え時に `load_worktrees_with_conflict_check()` 使用

**検証**: Worktreeビュー表示時にコンフリクトチェックが実行されること

### 3.2 並列コンフリクトチェック
- [ ] `src/tui/runner.rs`: 並列チェック実装
  - `JoinSet` で複数worktreeを同時チェック
  - エラーハンドリング
- [ ] パフォーマンステスト: 4つのworktreeで1秒未満

**検証**: 並列実行が正常動作すること、パフォーマンス要件を満たすこと

### 3.3 UIにコンフリクト表示
- [ ] `src/tui/render.rs`: `render_worktree_list()` にコンフリクトバッジ追加
  - `⚠{count}` 形式 (赤色、太字)
  - コンフリクトありの行を赤色でハイライト
- [ ] `src/tui/render.rs`: Mキーヒントの条件付き表示
  - コンフリクトなし且つブランチありの場合のみ表示

**検証**: コンフリクトありのworktreeに警告マークが表示されること、Mキーが無効化されること

### 3.4 自動リフレッシュ統合
- [ ] `src/tui/runner.rs`: Refresh taskでworktreeチェック追加
  - 5秒ごとに `load_worktrees_with_conflict_check()` 実行
  - `WorktreesRefreshed` イベント送信
- [ ] エラー時のフォールバック処理

**検証**: 自動リフレッシュが動作すること、エラーでクラッシュしないこと

### 3.5 包括的テスト
- [ ] 単体テスト: 全モジュールのカバレッジ確認
- [ ] 統合テスト: git操作の全パターン
- [ ] E2Eテスト: 全ユースケース
- [ ] パフォーマンステスト: 遅延測定

**検証**: `cargo test` 全パス、カバレッジ80%以上

### 3.6 ドキュメント更新
- [ ] AGENTS.md: Worktreeビュー機能の説明追加
- [ ] AGENTS.md: マージ機能とコンフリクト検出の説明
- [ ] AGENTS.md: トラブルシューティングセクション
- [ ] コード内コメント: 複雑なロジックに説明追加

**検証**: ドキュメントが正確で最新であること

---

## 最終検証

- [ ] `cargo fmt` でコードフォーマット
- [ ] `cargo clippy -- -D warnings` で警告なし
- [ ] `cargo test` で全テストパス
- [ ] TUIを起動して全機能の手動確認
  - View切り替え
  - Worktree作成・削除
  - エディタ・シェル起動
  - マージ (コンフリクトなし)
  - マージ拒否 (コンフリクトあり)
  - 自動リフレッシュ
- [ ] 既存機能の回帰テスト
  - Changesビューの全機能
  - 並列実行モード
  - エラーハンドリング

**完了条件**: 全チェック項目がパス、既存機能に影響なし

---

## 推定工数

- Phase 1: 8-10時間
- Phase 2: 6-8時間
- Phase 3: 4-6時間
- **合計**: 18-24時間

## 依存関係

- Phase 2 は Phase 1 完了後に開始
- Phase 3 は Phase 2 完了後に開始
- Phase 1 内のタスクは一部並列実行可能 (型定義 → Git実装 → AppState は並列可)

## 並列実行可能なタスク

- 1.1 (型定義) と 1.2 (Git実装) は並列可
- 1.5 (レンダリング) と 1.6 (キーバインド) は 1.3/1.4 完了後に並列可
- 2.1 (コンフリクト検出) と 2.2 (マージ) は並列可
