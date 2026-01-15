# Tasks: コマンド実行キューの追加

## Phase 1: CommandQueue モジュール実装

### Task 1.1: CommandQueue 構造体の定義

**ファイル**: `src/command_queue.rs` (新規作成)

**内容**:
- [ ] `CommandQueue` 構造体を定義
  - `config: CommandQueueConfig` フィールド
  - `last_execution: Arc<Mutex<Option<Instant>>>` フィールド
- [ ] `CommandQueueConfig` 構造体を定義
  - `stagger_delay_ms: u64`
  - `max_retries: u32`
  - `retry_delay_ms: u64`
  - `retry_error_patterns: Vec<String>`
- [ ] `Debug`, `Clone` trait を derive

**完了条件**:
- 構造体が定義され、コンパイルが通る
- 必要なフィールドがすべて含まれている

---

### Task 1.2: 時間差起動メソッドの実装

**ファイル**: `src/command_queue.rs`

**内容**:
- [ ] `execute_with_stagger()` メソッドを実装
  - 前回実行からの経過時間をチェック
  - 遅延時間未満の場合は待機
  - 実行時刻を更新
- [ ] `Instant::now()` と `tokio::time::sleep` を使用
- [ ] スレッドセーフな時刻管理（`Arc<Mutex<...>>`）

**完了条件**:
- メソッドが実装され、コンパイルが通る
- 連続実行時に遅延が適用される

---

### Task 1.3: エラーパターン検出と実行時間判定の実装

**ファイル**: `src/command_queue.rs`

**内容**:
- [ ] `is_retryable_error()` メソッドを実装
  - 設定されたエラーパターン（正規表現）とマッチング
  - `regex` クレートを使用
  - エラーハンドリング（無効な正規表現の処理）
- [ ] `should_retry()` メソッドを実装
  - 引数: `attempt`, `duration`, `stderr`, `exit_code`
  - 最大リトライ回数チェック
  - 終了コード0の場合は false
  - エラーパターンマッチング OR 実行時間が閾値未満
  - OR条件で判定

**完了条件**:
- メソッドが実装され、コンパイルが通る
- 正規表現マッチングが正しく動作する
- 実行時間判定が正しく動作する

---

### Task 1.4: 自動リトライメソッドの実装

**ファイル**: `src/command_queue.rs`

**内容**:
- [ ] `execute_with_retry()` メソッドを実装
  - ループでコマンド実行を試行
  - 実行開始時刻を `Instant::now()` で記録
  - 実行終了後、`duration = start.elapsed()` で実行時間を計測
  - エラー時に `should_retry(attempt, duration, stderr, exit_code)` でチェック
  - リトライ可能な場合は待機して再試行
  - 最大リトライ回数を超えた場合はエラー返却
- [ ] ログ出力（リトライ試行のログ、実行時間を含む）

**完了条件**:
- メソッドが実装され、コンパイルが通る
- リトライロジックが正しく動作する
- 実行時間が正しく計測される

---

### Task 1.5: CommandQueue のテスト

**ファイル**: `src/command_queue.rs`

**内容**:
- [ ] `test_stagger_delay` - 時間差起動のテスト
- [ ] `test_is_retryable_error_matches` - エラーパターンマッチのテスト
- [ ] `test_is_retryable_error_no_match` - マッチしない場合のテスト
- [ ] `test_retry_on_retryable_error` - エラーパターンマッチでリトライ
- [ ] `test_retry_on_short_duration` - 短時間実行でリトライ（パターンマッチなし）
- [ ] `test_no_retry_on_long_duration_without_pattern` - 長時間実行でリトライしない
- [ ] `test_retry_on_long_duration_with_pattern` - 長時間でもパターンマッチでリトライ
- [ ] `test_no_retry_on_success` - 成功時にリトライしないテスト
- [ ] `test_max_retries_exceeded` - 最大リトライ回数到達のテスト

**完了条件**:
- `cargo test command_queue` が通る
- すべてのテストケースがカバーされている
- 実行時間判定のテストが含まれている

---

## Phase 2: 設定の追加

### Task 2.1: デフォルト値の定義

**ファイル**: `src/config/defaults.rs`

**内容**:
- [ ] `DEFAULT_STAGGER_DELAY_MS` 定数を追加（値: 2000）
- [ ] `DEFAULT_MAX_RETRIES` 定数を追加（値: 2）
- [ ] `DEFAULT_RETRY_DELAY_MS` 定数を追加（値: 5000）
- [ ] `DEFAULT_RETRY_IF_DURATION_UNDER_SECS` 定数を追加（値: 5）
- [ ] `default_retry_patterns()` 関数を追加
  - デフォルトのエラーパターンリストを返す
  - `Cannot find module`, `ResolveMessage:`, `EBADF.*lock` など

**完了条件**:
- 定数と関数が定義され、コンパイルが通る
- デフォルト値が適切に設定されている

---

### Task 2.2: OrchestratorConfig への設定項目追加

**ファイル**: `src/config/mod.rs`

**内容**:
- [ ] `command_queue_stagger_delay_ms: Option<u64>` フィールドを追加
- [ ] `command_queue_max_retries: Option<u32>` フィールドを追加
- [ ] `command_queue_retry_delay_ms: Option<u64>` フィールドを追加
- [ ] `command_queue_retry_patterns: Option<Vec<String>>` フィールドを追加
- [ ] `command_queue_retry_if_duration_under_secs: Option<u64>` フィールドを追加
- [ ] デフォルト値を返すヘルパー関数を追加
- [ ] Serde の `#[serde(default)]` 属性を設定

**完了条件**:
- フィールドが追加され、コンパイルが通る
- JSONC パースが正しく動作する

---

### Task 2.3: 設定のテスト

**ファイル**: `src/config/mod.rs`

**内容**:
- [ ] `test_command_queue_config_defaults` - デフォルト値のテスト
- [ ] `test_command_queue_config_custom` - カスタム設定のテスト
- [ ] `test_parse_jsonc_with_command_queue` - JSONC パースのテスト

**完了条件**:
- `cargo test config::.*command_queue` が通る

---

## Phase 3: AgentRunner への統合

### Task 3.1: AgentRunner に CommandQueue を追加

**ファイル**: `src/agent.rs`

**内容**:
- [ ] `use crate::command_queue::{CommandQueue, CommandQueueConfig};` をインポート
- [ ] `AgentRunner` に `command_queue: CommandQueue` フィールドを追加
- [ ] `new()` メソッドで `CommandQueue` を初期化
  - 設定から `CommandQueueConfig` を構築
  - デフォルト値を使用

**完了条件**:
- フィールドが追加され、コンパイルが通る
- `AgentRunner::new()` が正しく初期化される

---

### Task 3.2: apply_command のキュー化

**ファイル**: `src/agent.rs`

**内容**:
- [ ] `run_apply_streaming()` メソッドを修正
  - `self.command_queue.execute_with_stagger()` を使用
- [ ] 時間差起動が適用されることを確認
- [ ] 既存のストリーミング機能を維持

**完了条件**:
- Apply コマンドで時間差起動が適用される
- 既存のテストが通る

---

### Task 3.3: archive_command のキュー化

**ファイル**: `src/agent.rs`

**内容**:
- [ ] `run_archive_streaming()` メソッドを修正
  - `self.command_queue.execute_with_stagger()` を使用
- [ ] 時間差起動とリトライ機構を適用

**完了条件**:
- Archive コマンドでキューが適用される
- 既存のテストが通る

---

### Task 3.4: resolve_command のキュー化

**ファイル**: `src/agent.rs`

**内容**:
- [ ] `run_resolve_streaming()` メソッドを修正
  - `self.command_queue.execute_with_retry()` を使用
- [ ] リトライ機構を適用（resolve は一時的エラーが多い）

**完了条件**:
- Resolve コマンドでキューが適用される
- 既存のテストが通る

---

### Task 3.5: その他コマンドのキュー化

**ファイル**: `src/agent.rs`, `src/parallel_run_service.rs`

**内容**:
- [ ] `analyze_command` の実行にキューを適用（`parallel_run_service.rs`）
- [ ] `worktree_command` の実行にキューを適用（該当箇所を特定）
- [ ] すべての `execute_shell_command*` メソッドでキューを使用

**完了条件**:
- すべてのコマンド実行でキューが適用される
- 既存のテストが通る

---

## Phase 4: 統合テスト

### Task 4.1: E2E テストの追加

**ファイル**: `tests/e2e_tests.rs` または新規ファイル

**内容**:
- [ ] `test_staggered_command_execution` - 時間差起動のE2Eテスト
  - 複数コマンドを連続実行し、遅延が適用されることを確認
- [ ] `test_retry_on_module_error` - リトライ機構のE2Eテスト
  - モジュールエラーを模擬し、リトライされることを確認
- [ ] `test_no_retry_on_permanent_error` - 永続エラーでリトライしないテスト

**完了条件**:
- `cargo test --test e2e_tests` が通る
- E2Eテストで実際のコマンド実行が検証される

---

### Task 4.2: 並列実行モードでの検証

**ファイル**: 手動テスト

**内容**:
- [ ] 並列実行モードで orchestrator を実行
  - `cargo run -- run --parallel --config .openspec-orchestrator.opencode.jsonc`
- [ ] 複数の変更を並列処理
- [ ] ログでキューの動作を確認
  - 時間差起動のログ
  - リトライのログ
- [ ] モジュール解決エラーの発生頻度を確認

**完了条件**:
- 並列実行時にエラー頻度が減少
- キューが正しく動作している

---

## Phase 5: ドキュメント更新

### Task 5.1: README の更新

**ファイル**: `README.md`

**内容**:
- [ ] コマンドキュー機能の説明を追加
- [ ] 設定例を追加
  ```jsonc
  {
    "command_queue_stagger_delay_ms": 2000,
    "command_queue_max_retries": 2,
    "command_queue_retry_delay_ms": 5000,
    "command_queue_retry_patterns": [
      "Cannot find module",
      "Lock acquisition failed"
    ]
  }
  ```

**完了条件**:
- ドキュメントが更新され、わかりやすい

---

### Task 5.2: AGENTS.md の更新

**ファイル**: `AGENTS.md`

**内容**:
- [ ] CommandQueue モジュールの説明を追加
- [ ] 設定オプションの説明を追加
- [ ] トラブルシューティング情報を追加

**完了条件**:
- AI agent が新機能を理解できる説明が追加されている

---

## 検証タスク

### Task V.1: 全テスト実行

**内容**:
- [ ] `cargo test` で全テスト通過
- [ ] `cargo clippy -- -D warnings` で警告無し
- [ ] `cargo fmt --check` でフォーマット問題無し

---

### Task V.2: 手動検証

**内容**:
- [ ] 実際の OpenCode 並列実行で動作確認
- [ ] エラー発生時のリトライ動作を確認
- [ ] ログ出力が適切か確認

---

## タスク実行順序

```
Phase 1: 1.1 → 1.2 → 1.3 → 1.4 → 1.5
         ↓
Phase 2: 2.1 → 2.2 → 2.3
         ↓
Phase 3: 3.1 → 3.2 → 3.3 → 3.4 → 3.5
         ↓
Phase 4: 4.1 → 4.2
         ↓
Phase 5: 5.1 → 5.2
         ↓
Verification: V.1 → V.2
```

**重要**: 各 Phase の終わりで `cargo test` と `cargo clippy` を実行し、既存機能が壊れていないことを確認すること。
