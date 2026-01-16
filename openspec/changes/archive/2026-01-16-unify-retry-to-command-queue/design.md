# Design: リトライ処理を command-queue に統一

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                     AgentRunner                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              CommandQueue                            │   │
│  │  ┌─────────────────┐  ┌────────────────────────┐   │   │
│  │  │ execute_with_   │  │ execute_with_retry_    │   │   │
│  │  │ stagger()       │  │ streaming()            │   │   │
│  │  │ (spawn only)    │  │ (spawn + stream + retry)│   │   │
│  │  └────────┬────────┘  └──────────┬─────────────┘   │   │
│  │           │                       │                 │   │
│  │           └───────────┬───────────┘                 │   │
│  │                       ▼                             │   │
│  │              ┌───────────────┐                      │   │
│  │              │ should_retry()│                      │   │
│  │              │ (判定ロジック)│                      │   │
│  │              └───────────────┘                      │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
              ┌───────────────────────────────┐
              │     parallel/executor.rs       │
              │  ┌─────────────────────────┐  │
              │  │ execute_apply_in_       │  │
              │  │ workspace()             │  │
              │  │ (リトライロジック削除)   │  │
              │  └─────────────────────────┘  │
              │  ┌─────────────────────────┐  │
              │  │ execute_archive_in_     │  │
              │  │ workspace()             │  │
              │  │ (リトライロジック削除)   │  │
              │  └─────────────────────────┘  │
              └───────────────────────────────┘
```

## Streaming Retry Design

### 問題

既存の `execute_with_retry()` は `wait_with_output()` を使用するため:
- コマンド完了まで出力が得られない
- TUI への逐次表示ができない

### 解決策

`execute_with_retry_streaming()` を新設:

```rust
/// Execute a command with automatic retry and streaming output
pub async fn execute_with_retry_streaming<F>(
    &self,
    mut command_fn: F,
    output_tx: Option<mpsc::Sender<OutputLine>>,
) -> Result<(ExitStatus, String)>  // (status, stderr for retry decision)
where
    F: FnMut() -> Command,
{
    let mut attempt = 0;

    loop {
        attempt += 1;
        let start_time = Instant::now();

        // Spawn with stagger
        let mut child = self.execute_with_stagger(|| command_fn()).await?;

        // Stream stdout/stderr while collecting stderr for retry decision
        let stderr = self.stream_output(&mut child, &output_tx).await?;

        // Wait for completion
        let status = child.wait().await?;
        let duration = start_time.elapsed();

        // Success case
        if status.success() {
            return Ok((status, stderr));
        }

        // Retry decision
        let exit_code = status.code().unwrap_or(-1);
        if !self.should_retry(attempt, duration, &stderr, exit_code) {
            return Err(OrchestratorError::AgentCommand(...));
        }

        // Notify retry via output channel
        if let Some(ref tx) = output_tx {
            tx.send(OutputLine::Stderr(format!(
                "[Retry {}/{}] Command crashed, retrying in {}ms...",
                attempt, self.config.max_retries, self.config.retry_delay_ms
            ))).await;
        }

        tokio::time::sleep(Duration::from_millis(self.config.retry_delay_ms)).await;
    }
}
```

### Streaming Helper

```rust
async fn stream_output(
    &self,
    child: &mut Child,
    output_tx: &Option<mpsc::Sender<OutputLine>>,
) -> Result<String> {
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let mut stderr_buffer = String::new();

    // Spawn tasks to read stdout/stderr concurrently
    let stdout_task = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Some(line) = lines.next_line().await? {
                if let Some(ref tx) = output_tx {
                    tx.send(OutputLine::Stdout(line)).await;
                }
            }
        }
        Ok::<(), std::io::Error>(())
    });

    let stderr_task = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Some(line) = lines.next_line().await? {
                stderr_buffer.push_str(&line);
                stderr_buffer.push('\n');
                if let Some(ref tx) = output_tx {
                    tx.send(OutputLine::Stderr(line)).await;
                }
            }
        }
        Ok(stderr_buffer)
    });

    stdout_task.await??;
    let stderr = stderr_task.await??;

    Ok(stderr)
}
```

## Retry Decision Logic

### 統一された判定基準

```rust
fn should_retry(&self, attempt: u32, duration: Duration, stderr: &str, exit_code: i32) -> bool {
    // Maximum retries check
    if attempt >= self.config.max_retries {
        return false;
    }

    // Success check (shouldn't happen but safety)
    if exit_code == 0 {
        return false;
    }

    // Condition 1: Error pattern match
    let matches_pattern = self.is_retryable_error(stderr);

    // Condition 2: Short execution (likely startup/environment issue)
    let is_short_execution = duration < Duration::from_secs(self.config.retry_if_duration_under_secs);

    // Condition 3: Agent crash (non-zero exit code) - NEW
    // All non-zero exits are considered crash candidates
    let is_crash = exit_code != 0;

    // Retry if: pattern match OR short execution OR crash
    matches_pattern || is_short_execution || is_crash
}
```

### 設定項目（既存）

| 設定 | デフォルト | 説明 |
|------|------------|------|
| `command_queue_max_retries` | 2 | 最大リトライ回数 |
| `command_queue_retry_delay_ms` | 5000 | リトライ間隔 |
| `command_queue_retry_if_duration_under_secs` | 5 | 短時間失敗の閾値 |
| `command_queue_retry_patterns` | (リスト) | リトライ対象パターン |

## executor.rs の変更

### Before (add-agent-crash-recovery)

```rust
// execute_apply_in_workspace()
if !status.success() {
    if iteration < MAX_ITERATIONS {
        warn!("Apply command crashed...");
        tokio::time::sleep(Duration::from_millis(2000)).await;
        continue;  // 独自リトライ
    }
    return Err(...);
}
```

### After (統一版)

```rust
// execute_apply_in_workspace()
let (status, _stderr) = command_queue
    .execute_with_retry_streaming(
        || build_apply_command(...),
        event_tx.map(|tx| /* adapt to OutputLine sender */),
    )
    .await?;

// リトライは command_queue 内で処理済み
// ここでは成功/最終失敗のみを処理
```

## Migration Path

1. **Phase 1**: `command_queue.rs` に `execute_with_retry_streaming()` を追加
2. **Phase 2**: `parallel/executor.rs` の apply リトライを置き換え
3. **Phase 3**: `parallel/executor.rs` の archive リトライを置き換え
4. **Phase 4**: `#[allow(dead_code)]` を削除、テスト更新

## Test Strategy

### Unit Tests

- `command_queue.rs` の既存テストを維持
- `execute_with_retry_streaming()` の新規テスト追加

### Integration Tests

- E2E テストでクラッシュリカバリー動作を確認
- Mock スクリプトで意図的にクラッシュさせてリトライを検証
