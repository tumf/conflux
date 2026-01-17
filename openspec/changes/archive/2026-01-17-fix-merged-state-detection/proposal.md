# Change: Fix merged state detection when changes remain

## Why
TUIの並列実行で、Archiveコミットがmainに存在するだけで「Merged」と判定され、実際には`openspec/changes/<change_id>`が残っているケースでも実行がスキップされます。これにより、TUIが「Processing 0%」のまま止まり、ユーザー操作と実際の状態が一致しません。

## What Changes
- Merged判定を「Archiveコミットがbaseに存在」かつ「changesディレクトリが消えている」場合に限定する
- Archiveコミットがあるのにchangesが残っている場合はMerged判定を行わず、通常の実行フローに戻す
- Merged判定でスキップする場合はTUI側の状態をMergedに更新し、0%停止を避ける

## Impact
- Affected specs: parallel-execution
- Affected code: src/execution/state.rs, src/parallel/mod.rs, src/tui/state/events.rs
