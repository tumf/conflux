# Proposal: Fix Processing Status on Task Completion

## Summary

TUIのステータス表示において、すべてのタスクが100%完了しても「Processing...」のまま更新されない問題を修正する。

## Problem Statement

現在の実装では、changeのすべてのタスクが完了（100%）しても、TUI上のステータスが「Processing...」のまま残り続ける問題がある。

**現象:**
```
│Current: increase-log-limit  |  Processing...  |  [████████████████████] 100.0% (29/29)
```

タスクが29/29（100%）完了しているにもかかわらず、ステータスは「Processing...」と表示され続ける。

## Root Cause Analysis

### 現在のフロー

1. **Phase 2（Apply）開始時**:
   - `ProcessingStarted` イベントが送信される
   - `queue_status` が `Processing` に設定される
   - `current_change` が設定される

2. **Apply成功後**:
   - ログメッセージ「Apply completed for {id}, checking for completion...」が送信される
   - **`ProcessingCompleted` イベントは送信されない** ← 問題箇所

3. **ループ再開 → Phase 1**:
   - タスクが100%完了していれば `archive_all_complete_changes` が呼ばれる
   - archive関数内で `ProcessingStarted` と `ProcessingCompleted` が送信される

### 問題の本質

`ProcessingCompleted` イベントは `archive_all_complete_changes` 関数内でのみ送信される（行1241）。
しかし、archiveが実行される前、またはarchiveに時間がかかる場合、UIは「Processing」のまま残る。

さらに、以下のケースでは `ProcessingCompleted` が送信されない可能性がある：
- archiveコマンドがエラーになった場合（`ProcessingError` が送信される）
- キャンセルされた場合

## Proposed Solution

### Option A: Apply完了時に `ProcessingCompleted` を送信（推奨）

Apply成功後、次のループに入る前に `ProcessingCompleted` イベントを送信する。
これにより、ユーザーは「タスク完了」と「archive待ち」の状態を区別できる。

**メリット:**
- シンプルな修正
- ユーザーに正確なステータスを提供

**デメリット:**
- archive前に「Completed」と表示される（実際にはarchive待ち）

### Option B: 新しいステータス `AwaitingArchive` を追加

タスク100%完了後、archive前の状態を表す新しいステータスを追加。

**メリット:**
- より正確なステータス表示
- archive処理中の状態が明確

**デメリット:**
- 実装が複雑
- 既存テストへの影響

### Option C: `ProgressUpdated` イベントで100%検知時に自動遷移

`handle_orchestrator_event` 内で、`ProgressUpdated` によりタスクが100%になった場合、
自動的に `Processing` から `Completed` に遷移する。

**メリット:**
- イベント送信側の変更不要

**デメリット:**
- UI側のロジックが複雑化
- 「Completed」の意味が曖昧になる（archive完了vs タスク完了）

## Recommendation

**Option A** を推奨。最もシンプルで、ユーザーの期待に沿った動作となる。

## Impact Assessment

- **影響範囲**: `src/tui.rs` のオーケストレーターループ
- **既存テスト**: 軽微な更新が必要（テストケースでの期待値変更）
- **下位互換性**: 問題なし

## References

- 関連コード: `src/tui.rs:1512-1539`（Apply成功後の処理）
- 関連イベント: `OrchestratorEvent::ProcessingCompleted`
