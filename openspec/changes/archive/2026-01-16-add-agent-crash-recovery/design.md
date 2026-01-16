# Design: AI エージェントクラッシュリカバリー

## 概要

AI エージェントが Apply/Archive コマンド実行中に異常終了した場合のリトライ機構を追加する。

## 現状のコード構造

### Apply コマンド実行フロー

```
execute_apply_in_workspace()
  └─ loop (iteration 1..max_apply_iterations)
       └─ spawn command
       └─ wait for completion
       └─ if !status.success() → return Err (即座にエラー返却)  ← 問題箇所
       └─ check task progress
       └─ if complete → break
```

### Archive コマンド実行フロー

```
execute_archive_in_workspace()
  └─ loop (attempt 1..ARCHIVE_COMMAND_MAX_RETRIES+1)
       └─ spawn command
       └─ wait for completion
       └─ if !status.success() → return Err (即座にエラー返却)  ← 問題箇所
       └─ verify_archive_completion()
       └─ if verification failed → continue (リトライ)
       └─ if verification success → break
```

## 修正設計

### Apply コマンド

**現在のコード** (`src/parallel/executor.rs` 533-538行目付近):

```rust
if !status.success() {
    return Err(OrchestratorError::AgentCommand(format!(
        "Apply command failed with exit code: {:?}",
        status.code()
    )));
}
```

**修正後**:

```rust
if !status.success() {
    if iteration <= max_apply_iterations {
        warn!(
            "Apply command crashed (iteration {}/{}), exit code: {:?}. Retrying in 2s...",
            iteration, max_apply_iterations, status.code()
        );
        tokio::time::sleep(Duration::from_millis(2000)).await;
        continue;  // 次のイテレーションへ
    }
    return Err(OrchestratorError::AgentCommand(format!(
        "Apply command failed after {} attempts with exit code: {:?}",
        iteration, status.code()
    )));
}
```

**理由**:
- Apply はすでに `max_apply_iterations` でループしているため、このループ内でリトライを継続
- 追加のリトライカウンターは不要

### Archive コマンド

**現在のコード** (`src/parallel/executor.rs` 973-978行目付近):

```rust
if !status.success() {
    return Err(OrchestratorError::AgentCommand(format!(
        "Archive command failed with exit code: {:?}",
        status.code()
    )));
}
```

**修正後**:

```rust
if !status.success() {
    if attempt <= ARCHIVE_COMMAND_MAX_RETRIES {
        warn!(
            "Archive command crashed (attempt {}/{}), exit code: {:?}. Retrying in 2s...",
            attempt, max_attempts, status.code()
        );
        tokio::time::sleep(Duration::from_millis(2000)).await;
        continue;  // 次の試行へ
    }
    return Err(OrchestratorError::AgentCommand(format!(
        "Archive command failed after {} attempts with exit code: {:?}",
        attempt, status.code()
    )));
}
```

**理由**:
- Archive はすでに `attempt` カウンターでループしているため、このループ内でリトライを継続
- 既存の verification リトライと統合

## 定数とパラメータ

| パラメータ | 値 | 根拠 |
|-----------|-----|------|
| リトライ待機時間 | 2000ms | 既存の `command_queue_stagger_delay_ms` と一致 |
| Apply 最大リトライ | `max_apply_iterations` | 設定済みの値を流用 |
| Archive 最大リトライ | `ARCHIVE_COMMAND_MAX_RETRIES` | 既存の定数を流用 |

## イベント通知

リトライ時に以下のログを出力:

```rust
warn!(
    "Apply command crashed (iteration {}/{}), exit code: {:?}. Retrying in 2s...",
    iteration, max_apply_iterations, status.code()
);
```

TUI へのイベント通知は既存の `ParallelEvent::ApplyOutput` / `ParallelEvent::ArchiveOutput` を使用（追加イベント不要）。

## テスト戦略

### ユニットテスト

- モックコマンドで exit code 1 を返し、リトライが発生することを確認
- 最大リトライ回数後にエラーが返却されることを確認
- リトライ後に成功した場合、正常終了することを確認

### E2E テスト

- 既存の E2E テスト (`tests/e2e_tests.rs`) に追加
- 一時的に失敗するスクリプトを用意し、リトライで回復することを確認

## 将来の拡張

1. **エラー履歴の AI への引き継ぎ**: リトライ時に前回のエラー情報をプロンプトに含める
2. **設定ファイルでのカスタマイズ**: `agent_crash_max_retries`, `agent_crash_retry_delay_ms` の追加
3. **リトライ対象のフィルタリング**: 特定の exit code のみリトライ
