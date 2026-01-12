# 設計: 並列実行時の経過時間表示修正

## アーキテクチャ概要

### イベント駆動の状態管理

TUI は `ExecutionEvent`（`OrchestratorEvent` のエイリアス）を受信し、`AppState` を更新します。経過時間の追跡は `ChangeState` 構造体の `started_at: Option<Instant>` フィールドで行われます。

```
Parallel Executor → ExecutionEvent → TUI State Handler → ChangeState Update → Render
```

### 現在のイベントフロー

**シリアル実行:**
```
ProcessingStarted (started_at ✓)
  → ArchiveStarted
  → ChangeArchived (elapsed_time ✓)
```

**並列実行（問題あり）:**
```
ApplyStarted (started_at ✗)
  → ApplyCompleted
  → ArchiveStarted (started_at ✗)
  → ChangeArchived (elapsed_time ✓)
```

### 提案されるイベントフロー

**並列実行（修正後）:**
```
ApplyStarted (started_at ✓ NEW)
  → ApplyCompleted
  → ArchiveStarted (started_at 保持)
  → ChangeArchived (elapsed_time ✓)
```

## コンポーネント設計

### 1. イベントハンドラ（`src/tui/state/events.rs`）

#### 新規: `ApplyStarted` ハンドラ

```rust
OrchestratorEvent::ApplyStarted { change_id } => {
    if let Some(change) = self.changes.iter_mut().find(|c| c.id == change_id) {
        // 未設定の場合のみ設定（冪等性）
        if change.started_at.is_none() {
            change.started_at = Some(Instant::now());
        }
        // 並列実行では Processing 状態に遷移
        change.queue_status = QueueStatus::Processing;
    }
    self.add_log(LogEntry::info(format!("Apply started: {}", change_id)));
}
```

**設計上の決定:**
- **冪等性**: `is_none()` チェックで重複設定を防止
- **状態遷移**: `Processing` に設定し、TUI 表示と一貫性を保つ
- **ログ記録**: デバッグ時のトレーサビリティ向上

#### 更新: `ArchiveStarted` ハンドラ

```rust
OrchestratorEvent::ArchiveStarted(id) => {
    if let Some(change) = self.changes.iter_mut().find(|c| c.id == id) {
        // 保険として未設定の場合のみ補完
        if change.started_at.is_none() {
            change.started_at = Some(Instant::now());
        }
        change.queue_status = QueueStatus::Archiving;
    }
    self.add_log(LogEntry::info(format!("Archiving: {}", id)));
}
```

**設計上の決定:**
- **防御的プログラミング**: 予期しないイベント順序への対処
- **最小変更**: 既存の動作を維持しつつ補完ロジックを追加

### 2. 状態構造（`src/tui/state/change.rs`）

既存の `ChangeState` 構造体をそのまま使用：

```rust
pub struct ChangeState {
    pub started_at: Option<Instant>,      // 処理開始時刻
    pub elapsed_time: Option<Duration>,   // 完了時の経過時間
    // ... 他のフィールド
}
```

**変更なし** - 既存のフィールドで十分対応可能。

### 3. 表示ロジック（`src/tui/render.rs`）

既存のロジックをそのまま使用：

```rust
let elapsed_text = if let Some(elapsed) = change.elapsed_time {
    format_duration(elapsed)              // 完了後は固定値
} else if let Some(started) = change.started_at {
    format_duration(started.elapsed())    // 処理中はリアルタイム計算
} else {
    "--".to_string()                      // 未開始
};
```

**変更なし** - `started_at` が設定されれば自動的に表示される。

## 状態遷移図

### 並列実行の状態遷移

```
NotQueued/Queued
    |
    | ApplyStarted (started_at 設定, status=Processing)
    v
Processing (経過時間表示開始)
    |
    | ApplyCompleted
    v
Completed
    |
    | ArchiveStarted (status=Archiving)
    v
Archiving (経過時間継続表示)
    |
    | ChangeArchived (elapsed_time 固定)
    v
Archived (経過時間固定表示)
```

### シリアル実行の状態遷移（既存）

```
NotQueued/Queued
    |
    | ProcessingStarted (started_at 設定, status=Processing)
    v
Processing (経過時間表示開始)
    |
    | ArchiveStarted (status=Archiving)
    v
Archiving (経過時間継続表示)
    |
    | ChangeArchived (elapsed_time 固定)
    v
Archived (経過時間固定表示)
```

## エラーハンドリング

### ケース 1: イベント順序の逸脱

**問題:** `ArchiveStarted` が `ApplyStarted` より先に到着
**対策:** `ArchiveStarted` で `started_at` を補完

### ケース 2: 重複イベント

**問題:** `ApplyStarted` が複数回発火
**対策:** `is_none()` チェックで最初の値を保持

### ケース 3: イベント損失

**問題:** `ApplyStarted` イベントが到達しない
**対策:** `ArchiveStarted` の補完ロジックがフォールバック

## テスト戦略

### 単体テスト（`src/tui/state/events.rs`）

1. **`test_apply_started_sets_started_at`**
   - `ApplyStarted` で `started_at` が設定される
   - `queue_status` が `Processing` になる

2. **`test_apply_started_idempotent`**
   - 既に `started_at` が設定されている場合、上書きしない

3. **`test_archive_started_preserves_started_at`**
   - `started_at` が既に設定されている場合、保持される

4. **`test_archive_started_fallback`**
   - `started_at` が未設定の場合、`ArchiveStarted` で補完される

5. **`test_parallel_elapsed_time_flow`**
   - `ApplyStarted` → `ArchiveStarted` → `ChangeArchived` の完全フロー
   - 経過時間が正しく記録される

### 統合テスト

手動での並列実行テスト：
1. 複数の変更を並列実行
2. TUI で経過時間が `--` でなく数値で表示されることを確認
3. archive 完了後も経過時間が保持されることを確認

## 実装順序

### Phase 1: イベントハンドラの追加
1. `ApplyStarted` ハンドラを実装
2. `ArchiveStarted` に補完ロジックを追加

### Phase 2: テストの追加
1. 単体テストを実装
2. テストの実行と検証

### Phase 3: 統合検証
1. 並列実行での動作確認
2. シリアル実行での回帰テスト

## パフォーマンス考慮事項

### メモリ使用量
- 影響なし（`Instant` は既存フィールド）

### CPU 使用量
- 影響なし（イベント処理にわずかな条件分岐を追加するのみ）

### レンダリング
- 影響なし（既存の表示ロジックを使用）

## セキュリティ考慮事項

- 該当なし（内部状態の管理のみ）

## 互換性

### 後方互換性
- ✅ 既存のシリアル実行フローは変更なし
- ✅ 既存の設定ファイルは変更不要
- ✅ 既存のテストは全てパス

### 前方互換性
- ✅ 将来的なイベント追加に対応可能な設計

## 代替設計の検討

### 代替案 A: イベントに timestamp を含める

**変更:**
```rust
ApplyStarted {
    change_id: String,
    timestamp: Instant,  // NEW
}
```

**却下理由:**
- イベント定義の変更が大規模
- イベント送信側（parallel executor）の変更が必要
- 現在の設計で十分対応可能

### 代替案 B: 新しい状態フィールド `apply_started_at`

**変更:**
```rust
pub struct ChangeState {
    pub apply_started_at: Option<Instant>,    // NEW
    pub archive_started_at: Option<Instant>,  // NEW
    pub elapsed_time: Option<Duration>,
}
```

**却下理由:**
- 複雑度が増加
- 表示ロジックの変更が必要
- 既存の `started_at` で十分

## まとめ

この設計は、最小限の変更で並列実行時の経過時間表示問題を解決します。既存のアーキテクチャとイベントシステムを活用し、シリアル実行への影響を回避しながら、並列実行での一貫した経過時間表示を実現します。
