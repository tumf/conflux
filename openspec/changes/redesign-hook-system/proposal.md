# Proposal: Redesign Hook System

## Summary

Hook システムを再設計し、明確で一貫性のあるライフサイクルモデルを提供する。

## Background

現在の hook システムには以下の問題がある：

### 問題1: iteration の定義が曖昧

- orchestrator.rs: 1 iteration = 1 apply または 1 archive
- tui.rs: 1 iteration = 1 apply のみ (archive は Phase 1 で別処理)
- 「iteration」が何を意味するか不明確

### 問題2: on_iteration_start に change_id がない

```rust
// orchestrator.rs line 232-237
let iter_start_context = HookContext::new(self.iteration, total_changes, queue_size, false);
// ↑ change_id なし

// この後で select_next_change() が呼ばれる
let next = self.select_next_change(&snapshot_changes).await?;
```

`on_iteration_start` は `select_next_change()` の**前**に呼ばれるため、`{change_id}` が使えない。

### 問題3: TUI と CLI で hook 実装が異なる

| Hook | orchestrator.rs | tui.rs |
|------|-----------------|--------|
| on_iteration_start | ✅ | ❌ |
| on_iteration_end | ✅ | ❌ |
| pre_apply | ✅ | ✅ |
| post_apply | ✅ | ✅ |

### 問題4: 「チェンジセット切り替え」を検知できない

ユースケース: jj で各チェンジセットごとに新しい change を作成したい

- `pre_apply` は毎回の apply 前に呼ばれる（同じチェンジセットで複数回）
- 「チェンジセットが切り替わったとき」に対応する hook がない

## Proposal

### 新しいループモデル

2層ループ構造を明確に定義する：

```
Run Loop (外側):
├── on_start
│
└── Change Loop (チェンジセット単位):
    ├── on_change_start  ← NEW: チェンジセット処理開始
    │
    ├── Apply Loop (apply 単位):
    │   ├── pre_apply
    │   ├── [apply execution]
    │   └── post_apply
    │
    ├── on_change_complete (タスク100%時)
    ├── pre_archive
    ├── [archive execution]
    ├── post_archive
    │
    └── on_change_end  ← NEW: チェンジセット処理終了
│
└── on_finish
```

### Hook 一覧（再設計後）

| Hook | タイミング | change_id |
|------|-----------|-----------|
| on_start | 実行開始時（1回） | ❌ |
| **on_change_start** | チェンジセット処理開始時 | ✅ |
| pre_apply | apply 実行前 | ✅ |
| post_apply | apply 成功後 | ✅ |
| on_change_complete | タスク100%完了時 | ✅ |
| pre_archive | archive 実行前 | ✅ |
| post_archive | archive 成功後 | ✅ |
| **on_change_end** | チェンジセット処理終了時 | ✅ |
| on_finish | 実行終了時（1回） | ❌ |
| on_error | エラー発生時 | ✅/❌ |

### 削除する Hook

- `on_first_apply` - `on_change_start` で代替可能（`{changes_processed}==0` をチェック）
- `on_iteration_start` - 意味が曖昧、`on_change_start` で代替
- `on_iteration_end` - 意味が曖昧、`on_change_end` で代替
- `on_queue_change` - 意図と実装が乖離していた（下記参照）

### 新規 Hook（TUI専用）

| Hook | タイミング | change_id |
|------|-----------|-----------|
| **on_queue_add** | ユーザーが Space でキューに追加時 | ✅ |
| **on_queue_remove** | ユーザーが Space でキューから削除時 | ✅ |
| **on_approve** | ユーザーが @ で承認時 | ✅ |
| **on_unapprove** | ユーザーが @ で承認取消時 | ✅ |

**補足**:
- 旧 `on_queue_change` は「archive でキューサイズが変わったとき」に呼ばれていた（orchestrator.rs）。本来の意図は「ユーザーがキュー操作したとき」だったため、`on_queue_add` / `on_queue_remove` に分離・明確化。
- 承認取消（unapprove）時にキューからも削除される場合、`on_unapprove` のみ呼ばれる（`on_queue_remove` は呼ばれない）。

### ユースケース対応

#### jj でチェンジセットごとに new する

```jsonc
{
  "hooks": {
    "on_change_start": "jj new -m 'changeset: {change_id}'"
  }
}
```

#### 各 apply 後にテストを実行

```jsonc
{
  "hooks": {
    "post_apply": "cargo test"
  }
}
```

## Impact

- **Breaking Change**: 旧 hook 名は削除（互換性考慮不要の指示に従う）
- configuration/spec.md の hook 関連要件を全面改訂
- orchestrator.rs と tui.rs の hook 呼び出しを統一

## Alternatives Considered

1. **状態管理スクリプト**: hook 内で前回の change_id を記録して比較
   - 却下理由: hook システム自体で解決すべき

2. **iteration の再定義**: iteration = チェンジセット単位に統一
   - 却下理由: 「iteration」より「change」の方が直感的

## Open Questions

None - 互換性考慮不要の指示により、クリーンな再設計が可能
