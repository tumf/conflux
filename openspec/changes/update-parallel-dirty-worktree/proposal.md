# Change: parallelモードで未コミット変更を許容

## Why
parallelモード実行時に作業ツリーがクリーンでないと実行できない制約が強すぎるため、警告を出した上で継続できるようにします。

## What Changes
- `--parallel` 実行時に未コミット/未追跡ファイルがあっても警告に切り替え、処理を継続する
- TUIのポップアップもエラーではなく警告として扱う

## Impact
- Affected specs: `parallel-execution`
- Affected code: `src/parallel_run_service.rs`, `src/parallel/executor.rs`, `src/tui/` 配下の並列実行UI
