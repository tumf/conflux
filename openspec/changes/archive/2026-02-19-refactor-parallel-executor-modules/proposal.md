# Change: Parallel executor module split

## Why
`src/parallel/mod.rs` が非常に大きく、構築・キュー管理・マージ調停など複数責務が同一ファイルに混在しています。責務分割を明確化し、保守性とテスト容易性を上げます。

## What Changes
- `ParallelExecutor` の構築・初期化・状態管理を責務ごとのサブモジュールに分割する
- `parallel/mod.rs` をモジュール宣言と再公開中心の入口に整理する
- 既存の公開 API と挙動は維持する（機能変更なし）

## Impact
- Affected specs: code-maintenance
- Affected code: `src/parallel/mod.rs`, `src/parallel/executor.rs`, `src/parallel/` 配下の新規サブモジュール

## Acceptance Criteria
- `cargo test` が成功する
- CLI/API の外部仕様変更がない
- 並列実行の挙動・ログの内容に回帰がない
