# Change: TUI state/events の責務分割

## Why
src/tui/state/events.rs が肥大化しており、イベント処理の責務が混在しているため変更の影響範囲が追跡しづらい。イベント種別ごとの処理分離が必要。

## What Changes
- AppState のイベント処理をイベント種別ごとに分割し、責務を明確化する。
- AppState 本体の構造は維持し、公開 API は変更しない。
- 既存挙動は変更せず、既存テストと追加テストで同一性を確認する。

## Impact
- Affected specs: code-maintenance
- Affected code: src/tui/state/events.rs, src/tui/state/mod.rs, src/tui/state/events/*
