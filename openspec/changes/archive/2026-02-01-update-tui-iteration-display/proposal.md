# Change: TUIのapplyingイテレーション表示を単調増加にする

## Why
TUIでapply中のchangeに対して、自動更新のタイミング次第で`applying:3`と`applying:4`が行き来することがあり、現在の進行度が正しく読み取れません。

## What Changes
- 共有オーケストレーション状態から取り込むイテレーション番号を、既存表示より小さい値で上書きしない
- 自動更新時のイテレーション番号は「より大きい値」を優先して表示する

## Impact
- Affected specs: `openspec/specs/tui-architecture/spec.md`
- Affected code: `src/tui/state/events/helpers.rs`, `src/tui/state/mod.rs`
