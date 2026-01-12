# 提案: パラレルモードの早期完了メッセージ修正

## 概要

パラレルモード実行時に、graceful stop（ESC キー）またはキャンセルで処理を停止した場合でも、「All changes processed successfully」という成功メッセージが表示される問題を修正する。

## 問題の詳細

### 現状の動作

1. TUI でパラレルモード実行を開始（F5）
2. すぐに ESC キーで graceful stop を実行
3. 実際には変更が一つも処理されていない（`[queued]` のまま）
4. しかし以下のログが表示される：
   ```
   11:42:27 Starting processing 3 change(s)
   11:42:27 Starting parallel processing of 3 change(s)
   11:42:27 Graceful stop: stopping parallel execution
   11:42:27 Processing stopped
   11:42:27 All parallel changes completed
   11:42:27 All changes processed successfully
   ```

### 根本原因

**ファイル**: `src/tui/orchestrator.rs`

1. **行 813-820**: Graceful stop 検出時
   - "Graceful stop: stopping parallel execution" ログを送信
   - `OrchestratorEvent::Stopped` を送信
   - ループを `break`

2. **行 960-966**: ループ終了後（無条件で実行）
   - "All parallel changes completed" ログを送信
   - `OrchestratorEvent::AllCompleted` を送信

**問題点**:
- ループは複数の理由で終了する（キャンセル、graceful stop、正常完了）
- しかし、終了理由に関わらず無条件で成功メッセージが送信される
- `Stopped` イベントの後に `AllCompleted` イベントが送信され、状態が矛盾する

### 期待される動作

| ループ終了理由 | 期待されるログと状態 |
|--------------|-------------------|
| キャンセル | "Parallel execution cancelled"（既存）<br>成功メッセージなし、`AllCompleted` イベント送信なし |
| Graceful stop | "Processing stopped"（既存）<br>成功メッセージなし、`AllCompleted` イベント送信なし |
| 正常完了（全成功） | "All parallel changes completed"<br>`AllCompleted` イベント送信 |
| 正常完了（一部エラー）| "Processing completed with errors"<br>`AllCompleted` イベント送信 |

## 提案される解決策

### 1. ループ終了理由の追跡

ループ内で終了理由を追跡するフラグを導入：
- `stopped_or_cancelled`: graceful stop またはキャンセルで終了したか
- `had_errors`: バッチ処理中にエラーが発生したか

### 2. 条件付き完了メッセージ送信

ループ終了後、終了理由に応じて適切なメッセージと イベントを送信：
- 停止/キャンセル時: 成功メッセージと `AllCompleted` を送信しない
- 正常完了（全成功）: 成功メッセージと `AllCompleted` を送信
- 正常完了（一部エラー）: 警告メッセージと `AllCompleted` を送信

## 影響範囲

- **変更ファイル**: `src/tui/orchestrator.rs`
- **影響する機能**: パラレルモード実行の完了処理
- **テスト**: E2E テストで停止/キャンセルケースを検証

## 関連仕様

- `parallel-execution`: パラレル実行の要件
- `tui-architecture`: TUI の状態管理とイベント処理

## 実装の優先度

**高**: ユーザーに誤解を与える不正確なステータスメッセージを修正する。
