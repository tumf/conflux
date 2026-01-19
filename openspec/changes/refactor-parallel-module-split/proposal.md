# Change: Parallel モジュール分割とテスト分離

## Why
src/parallel/mod.rs が肥大化しており、責務の追跡や変更の影響範囲が把握しづらい。テストも同一ファイルに集約されているため、変更時のレビューコストが高い。

## What Changes
- parallel モジュールの責務を整理し、ワークスペース管理や動的キューなどを専用モジュールに分割する。
- 並列実行のテストを専用サブモジュールに移動し、検証対象を分離する。
- 既存挙動は変更せず、既存テストと追加テストで同一性を確認する。

## Impact
- Affected specs: code-maintenance
- Affected code: src/parallel/mod.rs, src/parallel/executor.rs, src/parallel/tests/*, src/parallel/workspace.rs, src/parallel/dynamic_queue.rs, src/parallel/merge.rs
