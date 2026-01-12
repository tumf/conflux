# Proposal: Graceful Stop後のタスク状態修正

## 概要

現在、Graceful stop（Escキーで停止）を実行した後、実行中だったタスクが`Processing`状態のまま残り、進捗率が`[ 0%]`で表示され続け、スピナーアニメーションも動き続ける問題があります。

## 問題の詳細

### 現在の動作

1. ユーザーがRunningモード中にEscキーを押す
2. アプリケーションがStoppingモードに遷移し、graceful stopフラグを設定
3. Orchestratorが現在処理中の変更を完了後、`OrchestratorEvent::Stopped`イベントを送信
4. イベントハンドラがモードを`Stopped`に遷移するが、**Processing状態の変更はそのまま残る**
5. 結果：
   - リストに`[spinner] [ 0%]`のように表示され続ける
   - スピナーがメインループで更新され続けるため動き続ける

### 問題のコード箇所

**src/tui/state/events.rs:100-106**
```rust
OrchestratorEvent::Stopped => {
    self.mode = AppMode::Stopped;
    self.current_change = None;
    self.stop_mode = StopMode::None;
    if let Some(started) = self.orchestration_started_at {
        self.orchestration_elapsed = Some(started.elapsed());
    }
    self.add_log(LogEntry::warn("Processing stopped"));
}
```

モードは更新されるが、`Processing`や`Archiving`状態の変更がクリーンアップされていない。

### 比較：Force Stopの実装

Force stop（Stoppingモード中に2回目のEsc）では、適切にクリーンアップが実装されています：

**src/tui/runner.rs:244-249**
```rust
// Reset any in-flight change back to Queued
for change in &mut app.changes {
    if matches!(
        change.queue_status,
        QueueStatus::Processing | QueueStatus::Archiving
    ) {
        change.queue_status = QueueStatus::Queued;
    }
}
```

## 提案する解決策

`OrchestratorEvent::Stopped`のハンドリング時に、Force stopと同様の状態クリーンアップロジックを追加します。

### 変更内容

1. **Processing/Archiving状態の変更をクリーンアップ**
   - `OrchestratorEvent::Stopped`イベント受信時
   - Processing → Queued または NotQueued に遷移
   - Archiving → Queued に遷移

2. **経過時間の記録**
   - 中断された変更の経過時間を記録
   - 再開時の状態把握を容易にする

### 実装方針

**オプションA: Queuedに戻す（推奨）**
- 利点：再開（F5）時にすぐに処理を続行できる
- 欠点：なし
- Force stopと同じ動作で一貫性がある

**オプションB: NotQueuedに戻す**
- 利点：明示的に再選択が必要（より安全）
- 欠点：ユーザーが再選択する手間が増える

## 影響範囲

- **変更ファイル**: `src/tui/state/events.rs`
- **影響するモジュール**: TUIのイベントハンドリング
- **ユーザー体験**: Graceful stop後の表示が正しくなる
- **後方互換性**: なし（バグ修正）

## 代替案

### 代替案1: レンダリング時に特別処理

Stopped/Stoppingモード時は、Processing状態の変更を異なる方法で表示する。

**却下理由**: 
- 根本的な状態管理の問題を解決しない
- 内部状態と表示が乖離する

### 代替案2: 新しい状態を追加

`QueueStatus::Interrupted`などの新しい状態を追加する。

**却下理由**:
- 複雑性が増す
- Queuedで十分

## 検証方法

1. **手動テスト**
   - TUIを起動し、変更を処理開始
   - Escキーでgraceful stopを実行
   - 変更のステータスがQueuedに戻ることを確認
   - スピナーが停止することを確認
   - 進捗表示が適切になることを確認

2. **自動テスト**
   - `test_stopped_event_cleans_up_processing_changes`を追加
   - `test_stopped_event_records_elapsed_time`を追加

## 関連課題

- Force stopとGraceful stopの動作の一貫性
- TUI状態管理の改善

## 参考

- 既存のForce stop実装: `src/tui/runner.rs:244-249`
- 問題のイベントハンドラ: `src/tui/state/events.rs:100-106`
