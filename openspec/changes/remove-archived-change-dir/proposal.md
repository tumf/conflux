# Change: アーカイブ後のchangeディレクトリ削除

## Why
アーカイブ後に `openspec/changes/<change_id>/` が残留すると、アーカイブ検証や進捗判定が失敗し、アーカイブ処理の再実行ループが発生する。

## What Changes
- アーカイブ成功後に `openspec/changes/<change_id>/` をディレクトリごと削除する
- 削除に失敗した場合はアーカイブ検証を失敗とみなす
- アーカイブ済みの進捗取得を日付付きアーカイブディレクトリでも行えるようにする

## Impact
- Affected specs: parallel-execution
- Affected code: src/execution/archive.rs, src/parallel/executor.rs, src/tui/orchestrator.rs, src/task_parser.rs
