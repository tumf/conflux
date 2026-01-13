# 設計: パラレルモードの完了状態管理

## 問題分析

### 現在のアーキテクチャ

```
run_orchestrator_parallel()
├── loop {
│   ├── Check cancellation → break
│   ├── Check graceful stop → break
│   ├── Check batch_ids.is_empty() → break
│   ├── Process batch
│   └── Match result { Ok(_) | Err(_) } → continue
│   }
└── After loop (UNCONDITIONAL):
    ├── Send "All parallel changes completed"
    └── Send OrchestratorEvent::AllCompleted
```

**問題**: ループの終了理由に関わらず、無条件で成功メッセージが送信される。

### イベントフロー分析

#### 正常な場合

```
User: F5
  → run_orchestrator_parallel() start
  → Process all batches
  → loop exits (batch_ids.is_empty())
  → Send "All parallel changes completed"
  → Send AllCompleted
  → TUI: "All changes processed successfully"
```

#### Graceful stop の場合（現在のバグ）

```
User: F5, ESC
  → run_orchestrator_parallel() start
  → Check graceful_stop_flag (true)
  → Send Stopped event
  → break
  → Send "All parallel changes completed" ← 不適切
  → Send AllCompleted ← 不適切
  → TUI: "All changes processed successfully" ← 不適切
```

#### 期待される Graceful stop の動作

```
User: F5, ESC
  → run_orchestrator_parallel() start
  → Check graceful_stop_flag (true)
  → Send Stopped event
  → break
  → (何も送信しない)
  → TUI: "Processing stopped" のみ
```

## 設計上の決定

### 決定1: ループ終了理由のトラッキング

**アプローチ**: ローカルフラグで終了理由を追跡

**理由**:
- シンプルで明示的
- 既存のコード構造を大きく変更しない
- デバッグしやすい

**代替案（却下）**:
- 終了理由を表す enum を返す → 既存の `Result<()>` シグネチャを変更する必要がある
- 共有状態で管理 → 並行性の問題を引き起こす可能性

### 決定2: エラー状態の追跡

**アプローチ**: `had_errors` フラグでバッチエラーを累積

**理由**:
- バッチエラーが発生しても処理は継続される（現在の動作）
- 最終的な完了メッセージでエラーの有無を報告できる
- ユーザーに正確な状態を伝えられる

### 決定3: 条件付き完了イベント送信

**アプローチ**: `stopped_or_cancelled` フラグで `AllCompleted` 送信を制御

**理由**:
- `AllCompleted` は「すべての処理が試行された」ことを意味する
- 停止/キャンセル時は処理が中断されているため、`AllCompleted` は不適切
- `Stopped` イベントは既に送信済みなので、重複イベントを避ける

## 実装戦略

### ステップ1: フラグの導入

```rust
pub async fn run_orchestrator_parallel(...) -> Result<()> {
    // ... existing setup ...

    let mut stopped_or_cancelled = false;
    let mut had_errors = false;

    loop {
        // ... existing loop body ...
    }

    // ... conditional completion ...
}
```

### ステップ2: 終了パスでのフラグ設定

#### キャンセルパス（行 ~809）

```rust
if cancel_token.is_cancelled() {
    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::warn(
            "Parallel execution cancelled".to_string(),
        )))
        .await;
    stopped_or_cancelled = true;  // ← 追加
    break;
}
```

#### Graceful stop パス（行 ~820）

```rust
if graceful_stop_flag.load(Ordering::SeqCst) {
    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::info(
            "Graceful stop: stopping parallel execution".to_string(),
        )))
        .await;
    let _ = tx.send(OrchestratorEvent::Stopped).await;
    stopped_or_cancelled = true;  // ← 追加
    break;
}
```

#### エラーパス（行 ~948）

```rust
Err(e) => {
    had_errors = true;  // ← 追加
    let _ = tx
        .send(OrchestratorEvent::Log(LogEntry::error(format!(
            "Batch execution failed: {}",
            e
        ))))
        .await;
    // Continue to check for more changes even if this batch failed
}
```

### ステップ3: 条件付き完了処理（行 ~960-966 を置き換え）

```rust
if !stopped_or_cancelled {
    if had_errors {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::warn(
                "Processing completed with errors".to_string(),
            )))
            .await;
    } else {
        let _ = tx
            .send(OrchestratorEvent::Log(LogEntry::success(
                "All parallel changes completed".to_string(),
            )))
            .await;
    }
    let _ = tx.send(OrchestratorEvent::AllCompleted).await;
}
```

## テスト戦略

### E2E テストケース

#### ケース1: Graceful stop

```rust
#[tokio::test]
async fn test_parallel_graceful_stop_no_success_message() {
    // Setup: Start parallel execution with 3 changes
    // Action: Set graceful_stop_flag immediately
    // Assert: No "All parallel changes completed" in logs
    // Assert: No AllCompleted event received
    // Assert: "Processing stopped" in logs
}
```

#### ケース2: キャンセル

```rust
#[tokio::test]
async fn test_parallel_cancel_no_success_message() {
    // Setup: Start parallel execution with 3 changes
    // Action: Cancel via cancel_token
    // Assert: "Parallel execution cancelled" in logs
    // Assert: No AllCompleted event received
}
```

#### ケース3: エラー付き完了

```rust
#[tokio::test]
async fn test_parallel_completion_with_errors() {
    // Setup: Start parallel execution where one batch will fail
    // Action: Let execution complete
    // Assert: "Processing completed with errors" in logs
    // Assert: AllCompleted event received
}
```

### 手動テストチェックリスト

- [ ] パラレルモード、全成功 → "All parallel changes completed"
- [ ] パラレルモード、graceful stop → "Processing stopped" のみ
- [ ] パラレルモード、force cancel → "Force stopped" のみ
- [ ] パラレルモード、エラー発生 → "Processing completed with errors"
- [ ] シーケンシャルモード、影響なし（変更箇所が異なるため）

## リスク評価

### 低リスク

- **変更範囲が限定的**: 単一関数内の制御フロー変更
- **後方互換性**: イベントの順序や種類は変わらない（停止時のイベント削減）
- **副作用なし**: 他のモジュールへの影響なし

### 考慮事項

- **既存のテストへの影響**: 一部のテストが `AllCompleted` を期待している可能性
  - 対策: テストを見直し、停止ケースでは `AllCompleted` を期待しないよう修正

## 将来の拡張

### 統計情報の追加

現在は「エラーがあった」という二値フラグだが、将来的には：
- 成功した変更数
- 失敗した変更数
- スキップされた変更数

を追跡し、より詳細な完了メッセージを提供できる。

```rust
struct CompletionStats {
    total: usize,
    succeeded: usize,
    failed: usize,
    skipped: usize,
}
```

ただし、現時点では過剰な複雑化なので、シンプルなフラグ方式を採用。
