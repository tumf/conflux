# Change: TUIのresolve待ち表示を明確化

## Why
TUIでresolve待ちの変更がNotQueuedとして表示されると、状態が分かりづらく操作の判断を誤りやすい。
動作は正しいため、表示のみをresolve待ちとして明示することで利用者の理解を助けたい。

## What Changes
- resolve待ち（merge待機）中の変更を、TUI上でNotQueuedではなくresolve待ちとして表示する
- resolve待ち表示の状態遷移・保持条件を明確化する

## Impact
- Affected specs: tui-architecture
- Affected code: src/tui/state/events.rs, src/tui/state/mod.rs, src/tui/render.rs
