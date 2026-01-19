# Change: TUI stopped resume policy

## Why
STOPPED 状態から F5 で再開できないケースがあり、停止中の queue 状態と再開条件が曖昧です。停止時は実行マークのみを保持し、再開時に queued を復元する方針を明文化して挙動を一貫させます。

## What Changes
- Stopped へ遷移した時点で queue_status を NotQueued に戻し、実行マーク（[x]）を保持する方針を仕様化する
- Stopped 中の Space 操作は実行マークの付与/解除のみを行い、queue_status は NotQueued のまま維持する
- F5 再開時に実行マーク付き change を queued に復元して処理を再開する
- 強制停止時の取り扱いも同一方針で統一する

## Impact
- Affected specs: cli
- Affected code: src/tui/state/events.rs, src/tui/state/modes.rs, src/tui/runner.rs, src/tui/render.rs
