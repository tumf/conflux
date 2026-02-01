# Change: TUI Changes一覧の経過時間位置を更新

## Why
Changes一覧の経過時間が行末にあり、動作中スピナーと視線が離れて読み取りにくい。

## What Changes
- RunningモードのChanges一覧で、経過時間を動作中スピナーの直後に表示する
- 進行中ステータスの視認性を上げるため、経過時間をステータス表示の前に配置する

## Impact
- Affected specs: cli
- Affected code: src/tui/render.rs
