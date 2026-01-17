# Change: parallel apply/archive も CommandQueue 経由で実行する

## Why

parallel 実行では apply/archive が直接 `sh -c` で起動されており、CommandQueue の stagger/retry/streaming の挙動が適用されない。結果として同時起動が発生し、エージェント側のキャッシュ競合やリトライ不発の問題につながる。command-queue の仕様では「並列 apply/archive での stagger 適用」および「すべてのコマンドで統一動作」を求めているため、parallel 実行でも CommandQueue を必須経路にする。

## What Changes

- parallel の apply/archive 実行を CommandQueue 経由に切り替える
- shared stagger state を parallel executor 内で共有し、複数 worktree 間で遅延を揃える
- streaming 出力のリトライ通知を既存のログ/イベント経路で表示できるようにする

## Impact

- Affected specs: `command-queue`, `parallel-execution`
- Affected code:
  - `src/parallel/executor.rs`
  - `src/parallel/mod.rs`
  - `src/agent.rs`（共有 state の受け渡しに合わせて調整する場合）
