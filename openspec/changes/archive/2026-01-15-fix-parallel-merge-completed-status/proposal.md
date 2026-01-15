# 並列モードでマージ完了時のステータス表示を修正

## Why

並列モードTUIでマージ完了した変更が正しく `completed` として表示されないため、ユーザーが処理完了状態を正しく把握できない。この問題により、マージが成功しているにも関わらず、TUIでは未完了として表示され続けるため、再度実行しようとするなどの混乱が生じる可能性がある。

根本原因は、`MergeCompleted` イベントに変更IDが含まれていないため、TUIがどの変更のマージが完了したかを特定できず、ステータス更新ができないことにある。

## 問題

並列モードTUIで、正常にマージ完了した変更が `completed` ではなく `UNCOMMITTED` として表示される。

### 現象

- **期待動作**: 並列モードでマージが正常完了した変更は `completed` (緑色) として表示される
- **実際の動作**: マージ完了後も変更のステータスが更新されず、以前の状態 (おそらく `Processing` や `NotQueued`) のまま残る

## 根本原因

1. **並列モードでの個別マージ**: `src/parallel/mod.rs` で、各変更が archive 完了後、即座に個別にマージされる
2. **MergeCompleted イベント**: マージ成功時に `ParallelEvent::MergeCompleted { revision }` が送信される
3. **change_id の欠落**: このイベントには `change_id` が含まれていないため、TUIはどの変更がマージ完了したか特定できない
4. **TUIハンドラの欠落**: `src/tui/state/events.rs` に `MergeCompleted` イベントのハンドラがない

## What Changes

`MergeCompleted` イベントに `change_id` フィールドを追加し、TUIでこのイベントを処理して変更ステータスを `Archived` に更新する。

### 変更箇所

1. **src/events.rs**: `ExecutionEvent::MergeCompleted` に `change_id` フィールドを追加
2. **src/parallel/mod.rs**: `MergeCompleted` イベント送信時に `change_id` を含める (2箇所)
3. **src/tui/state/events.rs**: `MergeCompleted` イベントハンドラを追加し、ステータスを `Archived` に設定

## 期待される結果

並列モードで正常にマージ完了した変更が、TUIで `Archived` (= `completed`) ステータスとして正しく表示される。

## 影響範囲

- **破壊的変更なし**: `MergeCompleted` イベントにフィールドを追加するだけで、既存の動作は変わらない
- **テスト**: 既存のテストは全てパスする (イベント構造の変更のみ)
- **互換性**: イベントハンドラの追加により、TUIが並列モードのマージ完了を正しく認識できるようになる
