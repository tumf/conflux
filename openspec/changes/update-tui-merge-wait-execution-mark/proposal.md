# Change: TUIのmerge/resolve waitで実行マークを解除できるようにする

## Why
TUIが`Ready`表示でも内部的に`Running`のままになるケースがあり、`MergeWait`/`ResolveWait`の行で実行マーク（`[x]`）を外せません。待機状態の整理と再実行判断のため、実行マークの付け外しを許可する必要があります。

## What Changes
- `MergeWait`/`ResolveWait`の行でSpace操作による実行マークのトグルを許可する
- `MergeWait`/`ResolveWait`の行で@操作による承認状態のトグルを許可する（キュー状態やDynamicQueueは変更しない）
- Space/@によるDynamicQueueの変更は禁止のまま維持する（待機状態はキュー操作対象外）
- 既存のキュー操作（`NotQueued`/`Queued`の追加・削除）挙動は維持する

## Impact
- Affected specs: `tui-architecture`
- Affected code: `src/tui/state/guards.rs`, `src/tui/state/mod.rs`
