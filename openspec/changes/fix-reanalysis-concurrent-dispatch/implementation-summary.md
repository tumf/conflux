# 実装サマリ

## 変更概要

並列実行モードの re-analysis ループを `tokio::select!` ベースの非ブロッキングスケジューラに変更し、apply 実行中でも dynamic queue からの変更追加が即座に処理されるようにした。

## 主要な変更点

### 1. スケジューラ状態の追加（src/parallel/mod.rs:522-530）

```rust
// In-flight tracking
let mut in_flight: HashSet<String> = HashSet::new();

// Re-analysis trigger tracking
let mut reanalysis_reason = ReanalysisReason::Initial;

// Set needs_reanalysis to trigger first analysis
self.needs_reanalysis = true;
```

### 2. 非ブロッキングメインループ（src/parallel/mod.rs:534-815）

```rust
// Main scheduler loop: wait for triggers and dispatch changes
loop {
    // Step 1: Check for early exit conditions
    if self.needs_reanalysis && queued.is_empty() && in_flight.is_empty() {
        info!("All changes completed");
        break;
    }

    // Step 2: Dynamic queue ingestion
    if let Some(queue) = &self.dynamic_queue {
        while let Some(dynamic_id) = queue.pop() {
            if !queued.iter().any(|c| c.id == dynamic_id)
                && !in_flight.contains(&dynamic_id)
            {
                // Load change and add to queued
                queued.push(change);
                self.needs_reanalysis = true;
                reanalysis_reason = ReanalysisReason::Queue;
            }
        }
    }

    // Step 3: Re-analysis if needed
    if self.needs_reanalysis && !queued.is_empty() {
        info!(
            "Re-analysis triggered: iteration={}, queued={}, in_flight={}, trigger={}",
            iteration, queued.len(), in_flight.len(), reanalysis_reason
        );

        // Run dependency analysis
        let analysis_result = analyzer(&queued, iteration).await;

        // Calculate available slots based on in-flight count
        let available_slots = max_parallelism.saturating_sub(in_flight.len());
        info!(
            "Available slots: {} (max: {}, in_flight: {}, queued: {})",
            available_slots, max_parallelism, in_flight.len(), queued.len()
        );

        // Dispatch changes up to available slots
        for change_id in &analysis_result.order {
            if selected_changes.len() >= available_slots {
                break;
            }
            selected_changes.push(change_id.clone());
        }

        // Spawn tasks (non-blocking)
        for change_id in selected_changes {
            spawn_and_track_workspace(..., &mut in_flight, ...);
        }

        self.needs_reanalysis = false;
    }

    // Step 4: Wait for events using tokio::select!
    tokio::select! {
        // Queue notification
        Some(_) = dynamic_queue_notified => {
            // Will be processed in next iteration
        }
        // Debounce timer
        _ = tokio::time::sleep_until(debounce_until) => {
            self.needs_reanalysis = true;
        }
        // Task completion
        Some(result) = join_set.join_next() => {
            match result {
                Ok(workspace_result) => {
                    // Remove from in-flight
                    in_flight.remove(&workspace_result.change_id);

                    info!(
                        "Task completed: change='{}', in_flight={}, available_slots={}",
                        workspace_result.change_id,
                        in_flight.len(),
                        max_parallelism.saturating_sub(in_flight.len())
                    );

                    // Trigger re-analysis on next iteration
                    self.needs_reanalysis = true;
                    reanalysis_reason = ReanalysisReason::Completion;
                }
                Err(e) => {
                    error!("Task panicked: {:?}", e);
                }
            }
        }
        // Cancellation
        _ = cancel_token_cancelled => {
            info!("Cancellation requested");
            break;
        }
    }
}
```

### 3. In-flight 追跡（src/parallel/mod.rs:1757-1783）

```rust
async fn spawn_and_track_workspace(
    // ...
    in_flight: &mut HashSet<String>,
    // ...
) -> Result<()> {
    // Early return if already in-flight
    if in_flight.contains(&change_id) {
        return Ok(());
    }

    // ... workspace creation/reuse logic ...

    // Add to in-flight BEFORE spawning
    in_flight.insert(change_id.clone());

    // Spawn task (non-blocking)
    join_set.spawn(async move {
        // ... apply/acceptance/archive logic ...
        WorkspaceResult {
            change_id,
            workspace_name,
            error,
        }
    });

    Ok(())
}
```

### 4. Re-analysis トリガ種別（src/parallel/mod.rs:150-156）

```rust
/// Reason for triggering re-analysis (for logging and diagnostics)
#[derive(Debug, Clone, Copy)]
enum ReanalysisReason {
    Initial,
    Queue,
    Debounce,
    Completion,
}
```

## アーキテクチャ変更

### Before: ブロッキング dispatch

```
loop {
    analysis
    for change in order {
        workspace = create_workspace().await  // ブロッキング
        apply(workspace).await                // ブロッキング
        acceptance(workspace).await           // ブロッキング
        archive(workspace).await              // ブロッキング
    }
    // ここまで来るまで次の re-analysis が実行されない
}
```

### After: 非ブロッキング dispatch

```
loop {
    // Dynamic queue 取り込み
    while let Some(id) = queue.pop() {
        queued.push(id)
    }

    // Re-analysis（queued のみ対象）
    if needs_reanalysis {
        analysis_result = analyze(queued)
        available_slots = max - in_flight.len()

        for change in order.take(available_slots) {
            spawn(apply + acceptance + archive)  // 非ブロッキング
            in_flight.insert(change)
        }
    }

    // トリガ待機（複数同時待機）
    select! {
        _ = queue.notified() => { /* 次回 loop で処理 */ }
        _ = debounce_timer => { needs_reanalysis = true }
        result = join_set.join_next() => {
            in_flight.remove(result.change_id)
            needs_reanalysis = true
        }
        _ = cancel_token => { break }
    }
}
```

## ログ出力例（期待値）

```
[INFO] Re-analysis triggered: iteration=1, queued=2, in_flight=0, trigger=Initial
[INFO] Available slots: 4 (max: 4, in_flight: 0, queued: 2)
[INFO] Spawning change 'change-a' (workspace: ws-change-a)
[INFO] Spawning change 'change-b' (workspace: ws-change-b)
# ... apply 実行中 ...
[INFO] Queue changed, re-analysis triggered
[INFO] Re-analysis triggered: iteration=2, queued=1, in_flight=2, trigger=Queue
[INFO] Available slots: 2 (max: 4, in_flight: 2, queued: 1)
[INFO] Spawning change 'change-c' (workspace: ws-change-c)
# ... apply 完了 ...
[INFO] Task completed: change='change-a', in_flight=2, available_slots=2, error=None
[INFO] Re-analysis triggered: iteration=3, queued=0, in_flight=2, trigger=Completion
[INFO] Available slots: 2 (max: 4, in_flight: 2, queued: 0)
# ... 全て完了 ...
[INFO] All changes completed
```

## 検証ポイント

1. ✅ `tokio::select!` で複数トリガを同時待機
2. ✅ Dynamic queue 取り込みが re-analysis 前に実行
3. ✅ In-flight 数から available_slots を算出
4. ✅ Spawn は非ブロッキング（join_set で回収）
5. ✅ タスク完了で in_flight から削除し re-analysis トリガ

## 影響範囲

- **変更**: `src/parallel/mod.rs` - `execute_with_order_based_reanalysis` メソッド
- **追加**: `ReanalysisReason` 列挙型
- **追加**: `spawn_and_track_workspace` ヘルパー関数
- **削除**: なし（既存の同期的な実装を非同期化）

## 後方互換性

- ✅ 既存テスト全て成功（25 + 3 + 3 + 4 = 35 tests）
- ✅ CLI インターフェース変更なし
- ✅ 設定ファイルフォーマット変更なし
- ✅ イベント型定義変更なし
