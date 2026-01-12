# Tasks: CLI/TUI オーケストレーション統合

## Phase 1: 共通モジュール作成とアーカイブ統合

### 1.1 モジュール構造の作成
- [ ] 1.1.1 `src/orchestration/mod.rs` を作成
- [ ] 1.1.2 `src/orchestration/archive.rs` を作成
- [ ] 1.1.3 `src/lib.rs` または `src/main.rs` に `mod orchestration` を追加

### 1.2 アーカイブ処理の共通化
- [ ] 1.2.1 `ArchiveResult` enum を共通モジュールに定義
- [ ] 1.2.2 `archive_change()` 共通関数を実装
  - パス検証を正しいパス (`openspec/changes/archive/`) で実装
  - フック呼び出し (on_change_complete, pre_archive, post_archive) を含む
- [ ] 1.2.3 `archive_all_complete_changes()` を共通モジュールに移動
- [ ] 1.2.4 CLI の `Orchestrator::archive_change()` を共通関数呼び出しに変更
- [ ] 1.2.5 TUI の `archive_single_change()` を共通関数呼び出しに変更

### 1.3 テスト
- [ ] 1.3.1 `src/orchestration/archive.rs` のユニットテストを追加
- [ ] 1.3.2 `cargo test` で既存テストがパスすることを確認
- [ ] 1.3.3 `cargo clippy` でエラーがないことを確認

---

## Phase 2: Apply 処理の統合

### 2.1 OutputHandler トレイトの定義
- [ ] 2.1.1 `src/orchestration/output.rs` を作成
- [ ] 2.1.2 `OutputHandler` トレイトを定義
- [ ] 2.1.3 CLI 用の `LogOutputHandler` を実装
- [ ] 2.1.4 TUI 用の `ChannelOutputHandler` を実装

### 2.2 Apply 処理の共通化
- [ ] 2.2.1 `src/orchestration/apply.rs` を作成
- [ ] 2.2.2 `apply_change()` 共通関数を実装
  - フック呼び出し (pre_apply, post_apply, on_error) を含む
  - OutputHandler 経由で出力を処理
- [ ] 2.2.3 CLI の `Orchestrator::apply_change()` を共通関数呼び出しに変更
- [ ] 2.2.4 TUI のインライン apply 処理を共通関数呼び出しに変更

### 2.3 テスト
- [ ] 2.3.1 `src/orchestration/apply.rs` のユニットテストを追加
- [ ] 2.3.2 統合テストで CLI/TUI 両方の動作を確認

---

## Phase 3: 状態管理の統合

### 3.1 OrchestratorState 構造体の作成
- [ ] 3.1.1 `src/orchestration/state.rs` を作成
- [ ] 3.1.2 `OrchestratorState` 構造体を定義
  - `pending_changes: HashSet<String>`
  - `completed_changes: HashSet<String>`
  - `apply_counts: HashMap<String, u32>`
  - `iteration: u32`
  - `changes_processed: usize`

### 3.2 状態管理メソッドの実装
- [ ] 3.2.1 `mark_archived()`, `mark_pending()`, `increment_apply_count()` 等を実装
- [ ] 3.2.2 CLI で `OrchestratorState` を使用するよう修正
- [ ] 3.2.3 TUI で `OrchestratorState` を使用するよう修正

### 3.3 テスト
- [ ] 3.3.1 状態管理のユニットテストを追加

---

## Phase 4: フックコンテキストヘルパーの統合

### 4.1 ヘルパー関数の作成
- [ ] 4.1.1 `src/orchestration/hooks.rs` を作成
- [ ] 4.1.2 `build_archive_context()`, `build_apply_context()` 等を実装
- [ ] 4.1.3 CLI/TUI で共通ヘルパーを使用するよう修正

---

## Phase 5: 変更選択ロジックの統合（オプション）

### 5.1 選択ロジックの共通化
- [ ] 5.1.1 `src/orchestration/selection.rs` を作成
- [ ] 5.1.2 `select_next_change()` 共通関数を実装
  - LLM 分析をオプションとして受け取る
  - LLM なしの場合は進捗ベースのフォールバック
- [ ] 5.1.3 CLI/TUI で共通関数を使用

### 5.2 設定の追加
- [ ] 5.2.1 TUI でも LLM 分析を使用するかの設定項目を追加
- [ ] 5.2.2 ドキュメントを更新

---

## 検証タスク

- [ ] V.1 CLI モードで全機能が動作することを確認
- [ ] V.2 TUI モードで全機能が動作することを確認
- [ ] V.3 Parallel モードで全機能が動作することを確認
- [ ] V.4 `cargo test` で全テストがパス
- [ ] V.5 `cargo clippy -- -D warnings` でエラーなし
