# Change: 全ステートの進捗維持

## Why
各ステートで進捗が 0 と表示されることがあり、実際の tasks.md の状態と一致しません。進捗は全ステートで常に更新され、取得失敗を 0 件完了と誤認しない挙動に統一します。

## What Changes
- TUI で全ステートの tasks.md 由来の進捗を常に更新する
- Web 監視でも全ステートで completed を 0 に上書きしない
- 進捗取得失敗時は直前の表示を保持し、取得失敗を 0 件完了として扱わない

## Impact
- Affected specs: `openspec/specs/tui-architecture/spec.md`, `openspec/specs/web-monitoring/spec.md`
- Affected code: `src/tui/state/events.rs`, `src/tui/runner.rs`, `src/web/state.rs`
