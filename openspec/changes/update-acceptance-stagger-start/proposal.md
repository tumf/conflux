# Change: 並列 acceptance の開始をスタッガー共有に統一

## Why
並列実行時に acceptance コマンドが同時起動し、スタッガーが効かずエージェント起動エラーが増えるため。

## What Changes
- 並列実行で作成される `AgentRunner` が共有スタッガー状態を使うようにする。
- `acceptance_command` の起動が設定された遅延に従うことを保証する。

## Impact
- Affected specs: command-queue
- Affected code: `src/parallel/mod.rs`, `src/parallel/executor.rs`, `src/parallel/conflict.rs`, `src/agent/runner.rs`
