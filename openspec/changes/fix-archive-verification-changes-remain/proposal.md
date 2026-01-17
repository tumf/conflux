# Change: Archive verification fails when changes directory remains

## Why
Archive後に `openspec/changes/{change_id}` が残っているにも関わらず archive 検証が成功扱いとなり、後続の `ensure_archive_commit` で失敗して TUI にエラーが出る。検証が正しく失敗として扱われるようにし、未アーカイブ状態のまま次の処理へ進まないようにする。

## What Changes
- `verify_archive_completion` は `openspec/changes/{change_id}` が存在する場合、archive エントリの有無に関わらず未アーカイブとして扱う。
- 既存の「change が存在しない場合は成功」とする挙動は維持する。
- 並列/逐次/TUI の共通検証が同じ判定を共有する。

## Impact
- Affected specs: `parallel-execution`, `cli`
- Affected code: `src/execution/archive.rs`, `src/parallel/executor.rs`, `src/tui/orchestrator.rs`, `src/orchestration/archive.rs`
