# Proposal: リトライ処理を command-queue に統一

## Summary

`parallel/executor.rs` の独自リトライロジック（`add-agent-crash-recovery` で追加）を削除し、`command_queue.rs` の `execute_with_retry()` に統一する。

## Motivation

### 現状の問題

1. **リトライ実装の重複**:
   - `command_queue.rs`: `execute_with_retry()` メソッドが実装済みだが `#[allow(dead_code)]` で未使用
   - `parallel/executor.rs`: `add-agent-crash-recovery` で独自のリトライロジックを追加

2. **将来的な二重リトライのリスク**:
   - `command_queue.rs` のリトライを有効化すると、executor のリトライと二重に動作する可能性
   - 最悪ケース: `executor.max_iterations x command_queue.max_retries` 回のリトライ

3. **設計との不整合**:
   - `command-queue` の仕様では「すべてのコマンド種別への適用」として統一されたリトライ動作を要求
   - 現状は executor 固有のリトライ実装が存在

### 具体例

```rust
// command_queue.rs (未使用)
#[allow(dead_code)]  // <- 使われていない
pub async fn execute_with_retry<F>(&self, mut command_fn: F) -> Result<ExitStatus>

// parallel/executor.rs (add-agent-crash-recovery で追加)
if !status.success() {
    if iteration < MAX_ITERATIONS {
        tokio::time::sleep(Duration::from_millis(2000)).await;
        continue;  // 独自リトライ
    }
}
```

## Solution

### 概要

1. `command_queue.rs` の `execute_with_retry()` を有効化
2. streaming 対応版 `execute_with_retry_streaming()` を追加
3. `parallel/executor.rs` の独自リトライロジックを削除し、統一されたリトライ機構を使用

### 設計

#### Streaming 対応リトライ

`execute_with_retry()` は `wait_with_output()` を使用するため streaming に非対応。
新たに `execute_with_retry_streaming()` を追加:

```rust
pub async fn execute_with_retry_streaming<F>(
    &self,
    command_fn: F,
    output_tx: mpsc::Sender<OutputLine>,
) -> Result<ExitStatus>
where
    F: FnMut() -> Command,
```

- コマンド spawn 後、stdout/stderr を逐次 `output_tx` に送信
- コマンド失敗時、リトライ判定を実行
- リトライ時は再度 streaming を開始

#### リトライ判定の統一

以下のロジックを `command_queue.rs` に集約:

- エラーパターンマッチング（`Cannot find module` 等）
- 短時間実行判定（5秒未満）
- エージェントクラッシュ判定（exit code != 0）

### 対象コード

| ファイル | 変更内容 |
|----------|----------|
| `src/command_queue.rs` | `#[allow(dead_code)]` 削除、`execute_with_retry_streaming()` 追加 |
| `src/parallel/executor.rs` | 独自リトライロジック削除、`execute_with_retry_streaming()` 使用 |
| `src/agent.rs` | 必要に応じてリトライ経由に変更 |

## Scope

### In Scope

- `command_queue.rs` の `execute_with_retry()` 有効化
- streaming 対応版リトライメソッド追加
- `parallel/executor.rs` のリトライロジック統一
- 既存テストの維持・更新

### Out of Scope

- リトライ設定のカスタマイズ拡張（既存設定で対応）
- resolve コマンドのリトライ変更（既存実装で対応済み）
- 新しい設定項目の追加

## Risk

- **Streaming とリトライの複雑性**: リトライ時に既に送信した出力が再送される可能性
  - 対策: リトライ時に明示的な区切りメッセージを送信
- **既存テストへの影響**: `command_queue.rs` のテストは passing だが、統合テストが必要
  - 対策: E2E テストで動作確認

## Relationship to Other Changes

- **`add-agent-crash-recovery`**: この提案で追加されたリトライを削除し、統一された実装に置き換え
- **`command-queue` spec**: この提案により仕様との整合性が向上
