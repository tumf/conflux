# Proposal: add-global-resolve-lock

## 概要

並列実行モードにおいて、Resolve（コンフリクト解決/マージ）操作の同時実行を防ぐグローバルロックを導入する。

## 背景

### 現状の問題

現在の `ParallelExecutor` は `merge_lock: Arc<Mutex<()>>` フィールドを持ち、マージ操作をシリアライズしている。しかし、このロックはインスタンスごとに独立しているため、以下のケースで複数の Resolve が同時に実行される可能性がある：

1. **TUI モード**: `resolve_deferred_merge()` が新しい `ParallelExecutor` を毎回作成するため、ユーザーが複数の deferred change を連続で resolve しようとすると、それぞれが独立したロックを持つ
2. **Run モード**: 複数の worktree から base ブランチへのマージが同時に発火する可能性

### 影響

Resolve 操作は base ブランチ（メインブランチ）に対する変更を伴うため、複数が同時に実行されると：
- Git の状態が競合する
- マージコンフリクトが発生する
- 予期しないコミット履歴が作成される

## 提案する変更

### 方針

`std::sync::OnceLock` を使用したグローバルな `tokio::sync::Mutex` を導入し、すべての Resolve/マージ操作を単一のロックでシリアライズする。

### 変更内容

1. **グローバルロックの追加**: `src/parallel/mod.rs` にプロセス全体で共有されるグローバルマージロックを追加
2. **インスタンスロックの削除**: `ParallelExecutor` 構造体から `merge_lock` フィールドを削除
3. **ロック取得箇所の更新**: `attempt_merge()` でグローバルロックを使用するように変更
4. **テストコードの更新**: テスト内の `merge_lock` 初期化コードを削除

## 技術的詳細

### 実装

```rust
use std::sync::OnceLock;
use tokio::sync::Mutex;

/// Global lock for serializing all merge/resolve operations to base branch
static GLOBAL_MERGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn global_merge_lock() -> &'static Mutex<()> {
    GLOBAL_MERGE_LOCK.get_or_init(|| Mutex::new(()))
}
```

### 使用箇所

```rust
async fn attempt_merge(&self, ...) -> Result<MergeAttempt> {
    let _merge_guard = global_merge_lock().lock().await;
    // ... マージ処理
}
```

## スコープ外

- 複数プロセス間でのロック（ファイルロック等）は対象外
- 同一プロセス内での調停のみを対象とする

## リスクと軽減策

| リスク | 軽減策 |
|--------|--------|
| グローバルロックによるスループット低下 | Resolve 操作は元々逐次的であるべきなので影響は軽微 |
| テスト間の干渉 | `OnceLock` は初期化が一度のみなので、テストでも安全に動作 |

## 成功基準

- TUI から複数の deferred change を連続で resolve しても競合が発生しない
- Run モードで複数の worktree が同時に archive 完了しても、マージが順次実行される
- 既存のテストがすべてパスする
