# Design

## 目的
`MergeWait` の change に対して `M: resolve` を実行した際、解決処理の進行が UI に反映されず TUI が停止したように見える問題を解消する。

## 変更の要点

### 状態
- `QueueStatus` に `Resolving` を追加する。
- `MergeWait` →（M 実行）→ `Resolving` →（成功）→ `Archived`（現行挙動に合わせる）
- `MergeWait` →（M 実行）→ `Resolving` →（失敗）→ `MergeWait`

### 非同期実行
- `ResolveMerge` コマンドの処理は `tokio::spawn` 等でバックグラウンド実行する。
- 実行開始直後に UI 側の change 状態を `Resolving` に更新し、描画を継続する。

### イベント伝搬
- resolve の成功/失敗（change_id とエラー）を含むイベントを `ExecutionEvent` に追加し、TUI が `handle_orchestrator_event` で確実に反映できるようにする。
- resolve 中のログは既存のログ機構に流し込み、ユーザーに状況を見せる。

## 互換性
- 既存バリアントの意味やフィールドは変更せず、追加のみで実施する。
