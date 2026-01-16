# Design: add-global-resolve-lock

## 概要

並列実行モードにおける Resolve 操作のグローバルロック導入の設計。

## 現状のアーキテクチャ

```
┌──────────────────────────────────────────────────────────────┐
│                    ParallelExecutor (Instance 1)              │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  merge_lock: Arc<Mutex<()>>  ← インスタンス固有         │  │
│  └─────────────────────────────────────────────────────────┘  │
│                              │                                │
│                    attempt_merge()                            │
│                              ↓                                │
│                    resolve_and_merge()                        │
└──────────────────────────────────────────────────────────────┘

┌──────────────────────────────────────────────────────────────┐
│                    ParallelExecutor (Instance 2)              │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │  merge_lock: Arc<Mutex<()>>  ← 別のインスタンス         │  │
│  └─────────────────────────────────────────────────────────┘  │
│                              │                                │
│                    attempt_merge()  ← 同時実行可能！          │
└──────────────────────────────────────────────────────────────┘
```

### 問題のコードパス

```rust
// src/parallel/mod.rs:1689-1696
pub async fn resolve_deferred_merge(...) -> Result<()> {
    let mut executor = ParallelExecutor::new(...);  // 新規インスタンス作成
    executor.resolve_merge_for_change(change_id).await
}
```

TUI から呼び出されるたびに新しい `ParallelExecutor` が作成され、それぞれが独自の `merge_lock` を持つ。

## 提案するアーキテクチャ

```
┌────────────────────────────────────────────────────────────────────┐
│                         GLOBAL_MERGE_LOCK                          │
│               static OnceLock<Mutex<()>>                           │
│                    (プロセス全体で共有)                            │
└────────────────────────────────────────────────────────────────────┘
                                  ↑
         ┌────────────────────────┼────────────────────────┐
         │                        │                        │
┌────────┴────────┐    ┌─────────┴────────┐    ┌─────────┴────────┐
│ ParallelExecutor│    │ ParallelExecutor │    │ ParallelExecutor │
│   (Instance 1)  │    │   (Instance 2)   │    │   (Instance 3)   │
└────────┬────────┘    └─────────┬────────┘    └─────────┬────────┘
         │                       │                       │
         ↓                       ↓                       ↓
   attempt_merge()         attempt_merge()         attempt_merge()
         │                       │                       │
         └───────────────────────┼───────────────────────┘
                                 ↓
                    global_merge_lock().lock().await
                         (直列化される)
```

## 実装詳細

### 1. グローバルロックの定義

```rust
// src/parallel/mod.rs の先頭付近

use std::sync::OnceLock;

/// Global lock for serializing all merge/resolve operations to base branch.
///
/// This ensures that only one merge operation can modify the base branch
/// at any given time, regardless of which ParallelExecutor instance
/// initiates the operation.
static GLOBAL_MERGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Get the global merge lock, initializing it if necessary.
fn global_merge_lock() -> &'static Mutex<()> {
    GLOBAL_MERGE_LOCK.get_or_init(|| Mutex::new(()))
}
```

### 2. `ParallelExecutor` 構造体の変更

```rust
// Before
pub struct ParallelExecutor {
    // ...
    merge_lock: Arc<Mutex<()>>,  // 削除
    // ...
}

// After
pub struct ParallelExecutor {
    // ...
    // merge_lock フィールドを削除
    // ...
}
```

### 3. `attempt_merge()` の変更

```rust
// Before
async fn attempt_merge(&self, ...) -> Result<MergeAttempt> {
    let _merge_guard = self.merge_lock.lock().await;
    // ...
}

// After
async fn attempt_merge(&self, ...) -> Result<MergeAttempt> {
    let _merge_guard = global_merge_lock().lock().await;
    // ...
}
```

### 4. コンストラクタの変更

```rust
// Before
Self {
    // ...
    merge_lock: Arc::new(Mutex::new(())),
    // ...
}

// After
Self {
    // ...
    // merge_lock の初期化を削除
    // ...
}
```

### 5. テストコードの変更

```rust
// Before (各テスト内)
ParallelExecutor {
    // ...
    merge_lock: Arc::new(Mutex::new(())),
    // ...
}

// After
ParallelExecutor {
    // ...
    // merge_lock フィールドを削除
    // ...
}
```

## `OnceLock` vs `lazy_static` の選択理由

| 観点 | `OnceLock` | `lazy_static` |
|------|------------|---------------|
| 依存関係 | 標準ライブラリ（Rust 1.70+） | 外部クレート |
| 初期化 | 必要時に遅延初期化 | 必要時に遅延初期化 |
| スレッドセーフ | ✓ | ✓ |
| コード量 | 少ない | やや多い |

プロジェクトは Rust Edition 2021 を使用しており、`OnceLock` が利用可能なため、外部依存を増やさない `OnceLock` を採用する。

## 影響範囲

### 変更対象ファイル

- `src/parallel/mod.rs`
  - グローバルロック定義の追加
  - `ParallelExecutor` 構造体から `merge_lock` 削除
  - `attempt_merge()` でグローバルロックを使用
  - コンストラクタの更新
  - テストコードの更新

### 変更対象外

- `src/parallel/conflict.rs` - ロック取得は `attempt_merge()` 経由で行われるため変更不要
- `src/tui/runner.rs` - API は変更なし
- `src/main.rs` - API は変更なし

## テスト戦略

1. **既存テストの実行**: すべての既存テストがパスすることを確認
2. **手動テスト**: TUI から複数の deferred change を連続で resolve し、競合が発生しないことを確認
3. **ログ確認**: Resolve 操作が順次実行されていることをログで確認
