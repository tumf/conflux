# Change: TUIのiteration表示をステージ内で後退させない

## Why
archive/resolve の retry 中に TUI の iteration 表示が 2↔1 で切り替わることがあり、進行状況の把握が難しくなるため。

## What Changes
- ステージ開始時に iteration 表示をリセットする
- 出力イベントで iteration を更新する際、ステージ一致と単調増加の条件を適用する
- ステージが異なる/古いイベントが到着しても iteration 表示が後退しないようにする

## Impact
- Affected specs: `specs/tui-architecture/spec.md`
- Affected code: `src/tui/state.rs`, `src/tui/render.rs`（表示は現状のまま、更新条件のみ）
