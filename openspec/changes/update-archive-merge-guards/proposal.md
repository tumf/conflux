# Change: Archive/merge guardrails to prevent archived change resurrection

## Why
`openspec archive` の完了後に変更が `openspec/changes/` 側へ戻るケースがあり、`MergeWait` で停止する不整合が発生する。アーカイブ完了判定とマージ前検証を強化し、逆方向の移動を検知して再発を防ぐ。

## What Changes
- archive コミットの完了判定に `openspec/changes/<change_id>` の存在チェックを追加する。
- archive コミット作成フェーズで `openspec/changes/<change_id>` が残っている場合は失敗させる。
- merge 実行前に `verify_archive_completion` を再検証し、未アーカイブなら `MergeWait` に留める。

## Impact
- Affected specs: parallel-execution
- Affected code: `src/execution/archive.rs`, `src/parallel/mod.rs`
