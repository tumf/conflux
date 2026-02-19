# Change: リモートTUIに実行ログを配信する

## Why
サーバ側で実行は動作しているが、クライアント(TUI)にログが流れず進捗が見えないため。

## What Changes
- server の WebSocket で実行ログ(標準出力/標準エラー/状態ログ)を配信する
- remote TUI がログイベントを受信してログパネルへ反映する
- リモート実行中でも Change 行のログプレビューが更新される

## Impact
- Affected specs: `openspec/specs/server-mode/spec.md`, `openspec/specs/tui-architecture/spec.md`
- Affected code: `src/server/api.rs`, `src/server/runner.rs`, `src/remote/ws.rs`, `src/tui/orchestrator.rs`, `src/tui/state.rs`, `src/tui/events.rs`
