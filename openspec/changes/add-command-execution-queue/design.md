# Design: コマンド実行キュー

## アーキテクチャ概要

```
┌─────────────────────────────────────────────────┐
│           AgentRunner                           │
│  (apply/archive/resolve/analyze/worktree)       │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│         CommandQueue                            │
│  - Staggered start (時間差起動)                │
│  - Retry mechanism (自動リトライ)              │
│  - Error pattern detection (エラー検出)        │
└───────────────────┬─────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────┐
│       Tokio Process (Command::new)              │
│  - Shell command execution                      │
└─────────────────────────────────────────────────┘
```

## 主要コンポーネント

### 1. CommandQueue 構造体

```rust
pub struct CommandQueue {
    config: CommandQueueConfig,
    /// 最後のコマンド実行時刻（時間差制御用）
    last_execution: Arc<Mutex<Option<Instant>>>,
}

pub struct CommandQueueConfig {
    /// コマンド実行間の遅延（ミリ秒）
    pub stagger_delay_ms: u64,

    /// リトライ最大回数
    pub max_retries: u32,

    /// リトライ間の待機時間（ミリ秒）
    pub retry_delay_ms: u64,

    /// リトライ対象のエラーパターン（正規表現）
    pub retry_error_patterns: Vec<String>,

    /// この秒数未満の実行時間で失敗した場合、リトライ対象とする（デフォルト: 5秒）
    pub retry_if_duration_under_secs: u64,
}
```

### 2. 時間差起動の仕組み

```rust
async fn execute_with_stagger(&self, command: impl FnOnce() -> Command) -> Result<Child> {
    // 前回の実行からの経過時間をチェック
    let mut last = self.last_execution.lock().await;

    if let Some(last_time) = *last {
        let elapsed = last_time.elapsed();
        let delay = Duration::from_millis(self.config.stagger_delay_ms);

        if elapsed < delay {
            // まだ遅延時間が経過していない場合は待機
            let wait_time = delay - elapsed;
            tokio::time::sleep(wait_time).await;
        }
    }

    // 実行時刻を更新
    *last = Some(Instant::now());
    drop(last);

    // コマンド実行
    command().spawn()
}
```

### 3. 自動リトライの仕組み

```rust
async fn execute_with_retry(&self, mut command_fn: impl FnMut() -> Command) -> Result<ExitStatus> {
    let mut attempt = 0;

    loop {
        attempt += 1;

        // 時間差起動を適用してコマンド実行
        let start_time = Instant::now();
        let mut child = self.execute_with_stagger(|| command_fn()).await?;

        // 出力を収集
        let output = child.wait_with_output().await?;
        let duration = start_time.elapsed();

        // 成功の場合は終了
        if output.status.success() {
            return Ok(output.status);
        }

        // リトライ判定
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);
        let should_retry = self.should_retry(attempt, duration, &stderr, exit_code);

        // リトライ可能 && リトライ回数以内の場合
        if should_retry {
            warn!("Retryable error detected (attempt {}/{}), duration: {:?}s: {}",
                  attempt, self.config.max_retries, duration.as_secs_f64(), stderr);

            // リトライ前の待機
            tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
            continue;
        }

        // リトライ不可 or 最大回数到達
        return Err(/* エラー */);
    }
}

fn should_retry(&self, attempt: u32, duration: Duration, stderr: &str, exit_code: i32) -> bool {
    // 最大リトライ回数チェック
    if attempt >= self.config.max_retries {
        return false;
    }

    // 失敗していない場合はリトライ不要
    if exit_code == 0 {
        return false;
    }

    // 条件1: エラーパターンマッチ
    let matches_pattern = self.is_retryable_error(stderr);

    // 条件2: 実行時間が短い（一時的なエラーの可能性）
    let is_short_execution = duration < Duration::from_secs(self.config.retry_if_duration_under_secs);

    // リトライ判定: エラーパターンマッチ OR 実行時間が短い
    matches_pattern || is_short_execution
}

fn is_retryable_error(&self, stderr: &str) -> bool {
    self.config.retry_error_patterns.iter().any(|pattern| {
        Regex::new(pattern)
            .map(|re| re.is_match(stderr))
            .unwrap_or(false)
    })
}
```

## 統合ポイント

### AgentRunner への統合

```rust
pub struct AgentRunner {
    config: OrchestratorConfig,
    command_queue: CommandQueue,  // ← 追加
    apply_history: ApplyHistory,
    archive_history: ArchiveHistory,
}

impl AgentRunner {
    pub fn new(config: OrchestratorConfig) -> Self {
        let queue_config = CommandQueueConfig {
            stagger_delay_ms: config.command_queue_stagger_delay_ms
                .unwrap_or(DEFAULT_STAGGER_DELAY_MS),
            max_retries: config.command_queue_max_retries
                .unwrap_or(DEFAULT_MAX_RETRIES),
            retry_delay_ms: config.command_queue_retry_delay_ms
                .unwrap_or(DEFAULT_RETRY_DELAY_MS),
            retry_error_patterns: config.command_queue_retry_patterns
                .clone()
                .unwrap_or_else(default_retry_patterns),
            retry_if_duration_under_secs: config.command_queue_retry_if_duration_under_secs
                .unwrap_or(DEFAULT_RETRY_IF_DURATION_UNDER_SECS),
        };

        Self {
            config,
            command_queue: CommandQueue::new(queue_config),
            apply_history: ApplyHistory::new(),
            archive_history: ArchiveHistory::new(),
        }
    }

    // 既存のメソッドでキューを使用
    pub async fn run_apply_streaming(&self, change_id: &str) -> Result<...> {
        // キュー経由でコマンド実行
        self.command_queue.execute_with_retry(|| {
            // コマンド構築ロジック
        }).await
    }
}
```

## デフォルト設定値

```rust
// src/config/defaults.rs
pub const DEFAULT_STAGGER_DELAY_MS: u64 = 2000;            // 2秒
pub const DEFAULT_MAX_RETRIES: u32 = 2;                    // 最大2回リトライ
pub const DEFAULT_RETRY_DELAY_MS: u64 = 5000;              // 5秒待機
pub const DEFAULT_RETRY_IF_DURATION_UNDER_SECS: u64 = 5;   // 5秒未満の実行でリトライ

pub fn default_retry_patterns() -> Vec<String> {
    vec![
        // モジュール解決エラー
        r"Cannot find module".to_string(),
        r"ResolveMessage:".to_string(),
        // npm/bun レジストリエラー
        r"ENOTFOUND registry\.npmjs\.org".to_string(),
        r"ETIMEDOUT.*registry".to_string(),
        // ファイルロックエラー
        r"EBADF.*lock".to_string(),
        r"Lock acquisition failed".to_string(),
    ]
}
```

## リトライ判定ロジックの詳細

### 判定フロー

```rust
fn should_retry(attempt, duration, stderr, exit_code) -> bool {
    // ステップ1: 基本チェック
    if attempt >= max_retries { return false; }
    if exit_code == 0 { return false; }

    // ステップ2: OR条件による判定
    let matches_pattern = is_retryable_error(stderr);
    let is_short_execution = duration < Duration::from_secs(5);

    return matches_pattern || is_short_execution;
}
```

### 実行時間判定の理論的根拠

**仮説**: 実行時間が極端に短い失敗は一時的な環境問題の可能性が高い

| 実行時間 | 典型的なエラー種別 | 例 |
|---------|------------------|-----|
| < 1秒 | 起動失敗、環境エラー | モジュール未解決、ファイルアクセスエラー |
| 1-5秒 | 初期化エラー | レジストリ接続失敗、ロック競合 |
| > 5秒 | 論理エラー、テスト失敗 | 構文エラー、アサーション失敗 |

**デフォルト閾値（5秒）の根拠**:
- OpenCode/Claude Code の起動時間: 通常1-3秒
- 環境問題（node_modules更新など）による失敗: 0.5-2秒で発生
- テストや実装作業: 通常10秒以上かかる
- 5秒は起動フェーズと作業フェーズの境界として適切

### OR条件の動作例

| ケース | 実行時間 | パターンマッチ | 結果 | 理由 |
|-------|---------|--------------|------|------|
| 1 | 0.5秒 | ❌ | ✅ リトライ | 極端に短い → 環境問題 |
| 2 | 3秒 | ❌ | ✅ リトライ | 起動時の問題 |
| 3 | 30秒 | ✅ | ✅ リトライ | パターンマッチ |
| 4 | 30秒 | ❌ | ❌ リトライしない | 論理エラーの可能性 |
| 5 | 2秒 | ✅ | ✅ リトライ | 両条件満たす |

## トレードオフ

### 時間差起動

**利点**:
- 実装がシンプル
- 並列度を維持しつつ競合を減少
- 設定可能な遅延時間

**欠点**:
- 最初のコマンド実行までの待機時間が増加
- 厳密な排他制御ではない（競合の可能性は残る）

### 自動リトライ

**利点**:
- 一時的エラーから自動回復
- ユーザー介入不要
- 実行時間判定により、パターン登録不要なエラーもカバー

**欠点**:
- 失敗時のレイテンシ増加
- エラーパターンの保守が必要
- 閾値（5秒）が環境によっては不適切な可能性

### 実行時間判定

**利点**:
- エラーパターンに依存しない判定
- 未知のエラーもカバー
- シンプルな実装

**欠点**:
- 閾値の設定が環境依存
- 誤判定の可能性（短時間の論理エラーを誤ってリトライ）

## 代替案の検討

### 案1: ファイルロックによる排他制御
- **利点**: 厳密な排他制御
- **欠点**: 実装が複雑、デッドロックのリスク、並列度の低下
- **判断**: 時間差起動で十分対応可能なため不採用

### 案2: キャッシュディレクトリの分離
- **利点**: 根本的な解決
- **欠点**: 外部ツール（OpenCode）側の修正が必要、ディスク容量増加
- **判断**: 外部依存が大きいため不採用

### 案3: Semaphore による並列度制限
- **利点**: シンプルな実装
- **欠点**: 並列実行の恩恵が失われる
- **判断**: 並列度を維持する時間差起動を優先

## 拡張性

将来的に以下の機能を追加可能：

1. **動的な遅延調整**: エラー頻度に応じて遅延時間を自動調整
2. **優先度キュー**: 重要なコマンドを優先実行
3. **バックプレッシャー**: システムリソースに応じて実行を制御
