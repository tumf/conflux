# Change: parallel 実行の apply/archive も CommandQueue を経由する

## Why

parallel 実行の apply/archive は `CommandQueue` を経由していないため、stagger/retry が効かず同時起動が発生する。結果として OpenCode のキャッシュ競合（ENOENT）などが起き、同時多重起動時に失敗する。

既存の仕様では、apply/archive が exit code != 0 で終了した場合に自動リトライすることが求められているが、parallel 実装ではそれが満たされていない。serial/parallel で挙動を統一し、同時起動を抑制しつつリトライを有効化する。

## What Changes

- parallel 実行の apply/archive コマンド起動を CommandQueue 経由に変更する
- parallel 実行で stagger/retry の設定（`command_queue_*`）を適用する
- リトライ時のログ出力（"Retrying in 2s..." 等）を parallel 側でも可視化する

## Impact

- Affected specs: `parallel-execution`
- Affected code:
  - `src/parallel/executor.rs`（apply/archive の実行経路）
  - `src/parallel/mod.rs`（共有 CommandQueue の引き回し）
