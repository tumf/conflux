# Change: Webダッシュボードの状態取得を最新化

## Why
Webダッシュボードで承認状態のみが更新され、進捗やステータスが最新にならない問題があります。TUIの自動更新に依存した状態更新だけでは、Web側の手動リロードやポーリングが最新の状態を取得できないため、ユーザーが誤った状態を見続ける原因になります。

## What Changes
- Web APIが返す状態を、最新の変更情報（タスク進捗と承認状態）に同期する
- WebSocket初期送信とポーリングが最新状態を反映する
- 更新間隔による過剰なディスクI/Oを抑える仕組みを導入する

## Impact
- Affected specs: specs/web-monitoring/spec.md
- Affected code: src/web/state.rs, src/web/api.rs, src/web/websocket.rs
