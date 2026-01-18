# Change: Web UI を TUI と完全一致させるための監視アーキテクチャ再設計

## Why
Web UI が TUI と同じ情報をリアルタイムに反映できず、監視結果が不整合になるため、信頼できる監視基盤として再設計が必要です。

## What Changes
- Web UI が TUI と同一の状態モデルを購読できるように監視イベントと状態管理の構造を見直す
- TUI の更新ソース（ChangesRefreshed/実行イベント/キュー/ログ/ワークツリー）を Web へ一貫して配信できる統合経路を追加する
- Web UI のデータ契約を拡張し、TUI と同等の粒度で表示・更新できるようにする

## Impact
- Affected specs: web-monitoring, tui-architecture, observability
- Affected code: src/web/*, src/tui/*, src/events.rs, web/*
