# 並列モードでマージ完了状態を明確化する

## Why

並列モードと Serial モードで最終状態が区別されず、ユーザーが処理の完了状態を正しく把握できない。

### 問題の詳細

現在、TUI の `QueueStatus` enum には以下の状態が存在する：

- `NotQueued`, `Queued`, `Processing`, `Completed`, `Archiving`, `Archived`, `MergeWait`, `Resolving`, `Error`

しかし、**並列モードでマージ完了した変更も `Archived` 状態のままとなり、実際には main ブランチにマージ済みであることが視覚的に分からない**。

### Serial モードと Parallel モードの違い

- **Serial モード**: ベースブランチで直接作業 → `Archived` (git commit完了) が最終状態
- **Parallel モード**: ワークツリーで作業 → `Archived` (ワークツリーでcommit) → **`Merged`** (ベースブランチにマージ) という二段階

現在、`MergeCompleted` イベントは既に実装されているが（commit `2dd22d5`）、状態を `Archived` に設定するだけで、実際の「マージ完了」状態を表現できていない。

### 影響

- ユーザーが並列モードで変更が完全に統合されたかどうかを判別できない
- TUI 表示が実際の処理状態と乖離している
- `Archived` と `Merged` が同じ表示になり、進捗が不明確

## What Changes

`QueueStatus` enum に `Merged` 状態を追加し、並列モードでマージ完了時に明確に区別できるようにする。

### 変更内容

1. **`src/tui/types.rs`**: `QueueStatus::Merged` variant を追加
2. **`src/tui/state/events.rs`**: `MergeCompleted` イベント処理で `Merged` 状態に遷移
3. **`src/tui/render.rs`**: `Merged` 状態の表示ロジック追加
4. **Tests**: 新状態に対応するテストケース追加・更新

### 状態遷移（Parallel モード）

```
NotQueued → Queued → Processing → Completed
  → Archiving → Archived (ワークツリーでアーカイブ完了)
  → Merged (ベースブランチにマージ完了・最終状態)
```

### 後方互換性

- **Serial モード**: `Archived` が最終状態のまま（変更なし）
- **Parallel モード**: 新しい `Merged` 状態が最終状態として明確化
- **既存コード**: terminal state チェックに `Merged` を追加することで一貫性を保持

## 期待される結果

- 並列モードでマージ完了した変更が `Merged` 状態として明確に表示される
- Serial モードと Parallel モードの最終状態が視覚的に区別できる
- ユーザーが処理の完了度を正確に把握できる

## 影響範囲

- **破壊的変更なし**: Enum variant の追加のみ
- **Serial モード**: 影響なし（`Archived` が最終状態のまま）
- **Web UI**: 影響なし（独自の status システムを使用）
- **テスト**: 既存テストの期待値を更新
