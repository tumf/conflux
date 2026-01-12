# 提案: 並列実行時の経過時間表示修正

## 概要

並列実行モードでアーカイブ中の変更に対して経過時間が `--` と表示される問題を修正します。

## 背景

### 現在の問題

並列実行中、TUI の変更リストで「archiving」状態の変更の経過時間カラムに `--` が表示されます。これは、apply 開始から archive 完了までの全体の処理時間を知りたいユーザーにとって、進捗状況が把握できないという課題があります。

### 根本原因

**シリアル実行では：**
1. `ProcessingStarted` イベント → `started_at` を設定 ✓
2. `ArchiveStarted` イベント → 状態のみ更新
3. 経過時間は `started_at.elapsed()` で正しく表示される

**並列実行では：**
1. `ApplyStarted` イベント → **`started_at` が未設定** ✗
2. `ApplyCompleted` イベント
3. `ArchiveStarted` イベント → **`started_at` が未設定のまま** ✗
4. `ChangeArchived` イベント
5. 経過時間表示ロジックで `started_at` と `elapsed_time` の両方が `None` なので `--` を表示

### 該当コード

**イベント処理（`src/tui/state/events.rs`）：**
- `ProcessingStarted` のみが `started_at` を設定（L19-26）
- `ArchiveStarted` は `queue_status` のみ更新（L54-58）
- `ApplyStarted` イベントのハンドラが存在しない

**表示ロジック（`src/tui/render.rs:361-367`）：**
```rust
let elapsed_text = if let Some(elapsed) = change.elapsed_time {
    format_duration(elapsed)
} else if let Some(started) = change.started_at {
    format_duration(started.elapsed())
} else {
    "--".to_string()  // ← ここが表示される
};
```

## 提案される解決策

### アプローチ

並列実行の最初のイベント（`ApplyStarted`）で `started_at` を設定し、apply 開始から archive 完了までの全体の経過時間を追跡します。

### 変更内容

1. **`ApplyStarted` イベントハンドラを追加**
   - `started_at` を設定（未設定の場合のみ）
   - `queue_status` を `Processing` に更新
   - 適切なログエントリを追加

2. **`ArchiveStarted` に補完ロジックを追加（保険）**
   - `started_at` が未設定の場合のみ現在時刻を設定
   - 既存の動作（状態更新）は維持

3. **テストケースを追加**
   - `ApplyStarted` で `started_at` が設定されることを検証
   - `ArchiveStarted` で既存の `started_at` が保持されることを検証
   - 並列モードで apply から archive まで経過時間が正しく表示されることを検証

## 影響範囲

### 変更されるファイル
- `src/tui/state/events.rs` - イベントハンドラの追加・更新
- テストの追加・更新

### 影響を受けるコンポーネント
- TUI の経過時間表示（並列実行時のみ）
- イベント処理フロー（並列実行時のみ）

### 既存機能への影響
- **シリアル実行**: 影響なし（`ProcessingStarted` が引き続き `started_at` を設定）
- **並列実行**: 経過時間が正しく表示されるようになる（改善）

## 期待される効果

- ✅ 並列実行中の経過時間が apply 開始から表示される
- ✅ アーカイブ中も処理開始からの合計時間が表示される
- ✅ シリアル/並列実行で一貫した経過時間表示
- ✅ ユーザーが処理の進捗を把握しやすくなる

## 代替案

### 代替案 A: `ArchiveStarted` でのみ補完
- **メリット**: 最小限の変更
- **デメリット**: archive 開始からの時間のみになり、apply の時間が含まれない
- **却下理由**: ユーザーは apply 開始から終了までの全体時間を知りたい

### 代替案 B: 新しい経過時間計算ロジック
- **メリット**: 表示ロジックのみ変更
- **デメリット**: イベント時刻の記録がないと計算できない
- **却下理由**: 根本的な解決にならない

## 依存関係

- なし（既存のイベントシステムを使用）

## リスク評価

### 低リスク
- 既存の動作を変更せず、未設定の場合のみ補完
- 並列実行のイベント順序は保証されている
- シリアル実行への影響なし

### 潜在的な問題
- 重複設定の可能性 → 条件チェックで回避（`is_none()` チェック）
- テストカバレッジ → 包括的なテストケースで対応

## 検証計画

1. 並列実行で変更を処理し、経過時間が表示されることを確認
2. シリアル実行で既存の動作が維持されることを確認
3. 単体テストで全てのイベントパターンをカバー
4. エッジケース（停止/再開など）での動作確認
