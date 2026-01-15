# TUIモードでのWebState更新イベント転送の実装

## 概要

TUIモード（`tui --web`）で並列実行時に、WebSocketクライアントへリアルタイム更新が送信されない問題を修正する。

## 問題

現状、以下の動作になっている：

1. **CLIモード (`run --web`)**: 正常動作
   - `orchestrator.rs` でWebState更新ループが設定される
   - 並列実行イベントが `web_state.apply_execution_event()` で処理される
   - WebSocketクライアントがリアルタイム更新を受信する

2. **TUIモード (`tui --web`)**: 動作しない
   - `main.rs` でWebStateを作成し、WebSocketサーバーを起動する
   - しかし、`run_tui()` には `web_url` (文字列) のみ渡される
   - `run_orchestrator_parallel()` にWebStateの参照がない
   - 並列実行イベントがWebStateに送信されない
   - WebSocketは接続されるが、更新が送信されない

## 根本原因

TUIモードでは、WebStateインスタンスが作成されているが、並列実行のイベントハンドラーに接続されていない。

- `main.rs:103-105` でWebStateを作成し、WebSocketサーバーを起動
- `run_tui()` には `web_url` しか渡していない
- `run_orchestrator_parallel()` にWebStateの参照がないため、イベントを送信できない

## 解決策

TUIモードでも、CLIモードと同様にWebStateへのイベント転送を実装する。

### 変更が必要な箇所

1. **src/main.rs** (Line 102-134)
   - `web_state` Arc<WebState> を `run_tui()` に渡す

2. **src/tui/runner.rs** (`run_tui`, `run_tui_loop`)
   - `web_state: Option<Arc<WebState>>` 引数を追加
   - `run_orchestrator_parallel()` 呼び出し時に渡す

3. **src/tui/orchestrator.rs** (`run_orchestrator_parallel`)
   - `web_state: Option<Arc<WebState>>` 引数を追加
   - CLIモード (`orchestrator.rs:828-842`) と同様のイベント転送ループを追加

## 期待される効果

- TUI + Web監視 + 並列実行モードで、WebUIがリアルタイム更新を受信できる
- CLIモードとTUIモードで一貫した動作になる
- WebSocket接続が「connected」状態でステータスが更新されるようになる

## 影響範囲

- TUIモードでのWeb監視機能のみ
- CLIモードには影響なし
- 既存のWebSocket/WebState実装には変更なし

## リスク

- 低リスク（既存の動作パターンをTUI側に適用するのみ）
- イベント転送タスクの停止条件のみ注意が必要（CLIと同じ実装で問題なし）

## 過去の試行

以下のコミットで同様の問題に対して修正が試みられているが、TUI側のイベント転送が実装されていなかった：

- `dcb327b` Merge change: fix-web-monitoring-parallel-status-updates (最新)
- `d9df0c3` Merge change: fix-web-monitoring-parallel-status-updates (前回)
- `0513ce0` Archive: fix-web-monitoring-parallel-status-updates (さらに前)

これらの修正では、WebSocket実装やブロードキャスト処理の改善が行われたが、TUIモードでのイベント転送経路の欠落は解決されていなかった。
