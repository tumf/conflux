# Proposal: sync-tui-logs-to-debug-file

## 概要

TUI Logs Viewに表示されるログエントリを、デバッグログファイル（`--logs`オプション指定時）にも同期出力する。

## 背景

現在、TUIのログシステムには2つの独立した出力先が存在する:

1. **TUI Logs View (画面表示)**: `LogEntry`を`AppState.logs`に追加し、TUIで表示
2. **デバッグログファイル**: `tracing`クレートを使用し、`--logs`オプション指定時にファイル出力

問題点として、エージェントの重要なエラーメッセージ（例: "Apply failed for X: Y"）は`LogEntry::error()`でTUI画面には表示されるが、`tracing::error!()`が呼ばれていないためデバッグログファイルには出力されない。

これにより、TUI画面に表示されているエラーをログファイルで確認できず、デバッグや事後分析が困難になっている。

## 解決策

`LogEntry`を`add_log()`メソッドで追加する際に、そのレベルに応じた`tracing`マクロも同時に呼び出す。

具体的には:
- `LogEntry` (White) → `tracing::info!()`
- `LogEntry` (Green/success) → `tracing::info!()`
- `LogEntry` (Yellow/warn) → `tracing::warn!()`
- `LogEntry` (Red/error) → `tracing::error!()`

## スコープ

- `src/tui/state/logs.rs`の`add_log()`メソッドを修正
- `LogEntry`にレベル情報を追加（色だけでなく明示的なログレベル）
- 既存の動作への影響は最小限（`--logs`未指定時は`tracing`サブスクライバーが未初期化のため出力なし）

## 影響範囲

- `src/events.rs` - `LogEntry`構造体にログレベルフィールド追加
- `src/tui/state/logs.rs` - `add_log()`でtracing出力を追加
