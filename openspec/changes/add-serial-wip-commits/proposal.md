# Change: apply反復ごとのWIPコミット

## Why
- apply反復ごとに作業状態を確実に残し、失敗時も再開しやすくするため
- WIPを最終マージ時に1つへsquashする前提を明確化するため

## What Changes
- 逐次（非parallel）apply反復ごとに `--allow-empty` のWIPコミットを作成する
- parallel実行でも各反復で新規WIPコミットを作成し、apply失敗時もスナップショットを残す
- apply完了時にWIPを単一のApplyコミットへsquashする手順を明示する

## Impact
- Affected specs: cli, parallel-execution
- Affected code: `src/orchestrator.rs`, `src/parallel/executor.rs`, `src/vcs/git/mod.rs`, `src/execution/apply.rs`
