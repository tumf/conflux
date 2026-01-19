# Change: TUI stopped queue policy

## Why
TUI の STOPPED 状態で queue の意味が曖昧になっており、ユーザーの期待（実行中のみ queued）と現行挙動が一致していません。停止時の状態遷移を明確化し、再開時の挙動を一貫させます。

## What Changes
- Stopped へ遷移した時は queued を維持せず、queue 状態を not queued に戻す方針を明文化する
- F5 の再開時に、実行マーク（x）の付いた change を queued に復元してから実行を開始する
- 強制停止（Esc 2 回）時の queue 状態も同じポリシーに統一する

## Impact
- Affected specs: cli
- Affected code: src/tui/state/events.rs, src/tui/state/modes.rs, src/tui/runner.rs, src/tui/render.rs
