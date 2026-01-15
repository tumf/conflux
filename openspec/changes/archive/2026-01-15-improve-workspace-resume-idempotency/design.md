# Design: Workspace Resume Idempotency

## アーキテクチャ概要

### 状態遷移モデル

```
                    ┌─────────┐
                    │ Created │
                    └────┬────┘
                         │
                    apply starts
                         │
                         ▼
                  ┌──────────┐
           ┌──────│ Applying │◄──────┐
           │      └────┬─────┘       │
           │           │              │
    apply succeeds  WIP commits    apply retries
           │           │              │
           │           └──────────────┘
           │
           ▼
      ┌─────────┐
      │ Applied │
      └────┬────┘
           │
     archive runs
           │
           ▼
     ┌──────────┐
     │ Archived │
     └────┬─────┘
           │
      merge runs
           │
           ▼
      ┌────────┐
      │ Merged │ ───► Skip & Cleanup
      └────────┘
```

### 状態検出アルゴリズム

**優先順位（上から順にチェック）:**

1. **Merged**: `git log --merges` で `"Merge change: {change_id}"` を検索
   - 見つかった → `WorkspaceState::Merged`
   - 理由：最終状態なので最優先で確認

2. **Archived**: `is_change_archived()` && `is_archive_commit_complete()`
   - 両方true → `WorkspaceState::Archived`
   - 理由：マージ前の最終段階

3. **Applying**: WIPコミット（`WIP: {change_id} (N/M tasks, apply#K)`）を検索
   - 見つかった → `WorkspaceState::Applying { iteration: K, completed: N, total: M }`
   - 理由：Apply中断からの再開に必要な情報

4. **Applied**: Applyコミット（`Apply: {change_id}`）を検索
   - 見つかった → `WorkspaceState::Applied`
   - 理由：Archive前の状態

5. **Created**: デフォルト
   - どれにも該当しない → `WorkspaceState::Created`

## 主要コンポーネント

### 1. 状態検出モジュール (`src/execution/state.rs`)

新規モジュールとして作成。WorkspaceState enumと4つの検出関数を提供。

### 2. Resume処理の修正 (`src/parallel/mod.rs`)

既存の `execute_group()` 関数（line ~680）の再開処理を状態ベースに変更。

## トレードオフ

### メリット

1. **冪等性保証** - 同じ状態で複数回実行しても安全
2. **無駄な処理削減** - 完了済みステージをスキップ
3. **デバッグ容易性** - 明確な状態定義により問題の切り分けが簡単
4. **手動介入との親和性** - ユーザーが手動でcommit/mergeしても正しく検出

### デメリット

1. **複雑性増加** - 状態検出ロジックが追加（約+200行）
2. **Git操作コスト** - `git log` を複数回実行（ワークスペース数が多い場合オーバーヘッド）
3. **テスト負荷** - 5状態 × 各種遷移パターンのテストが必要
4. **後方互換性リスク** - 既存のワークスペースが新しい状態検出に適合しない可能性

## 実装順序

1. **Phase 1**: 状態検出関数（`src/execution/state.rs`）
2. **Phase 2**: Resume処理修正（`src/parallel/mod.rs`）
3. **Phase 3**: Integration tests
4. **Phase 4**: Documentation
