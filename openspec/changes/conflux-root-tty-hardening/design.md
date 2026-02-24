---
title: 設計 - Conflux 側の根本対策 (TTY停止/STAT=T 対策)
---

## 方針

Conflux が起動する子プロセス（特に `sh -c ...` でパイプラインを実行するケース）について、
Unix では pre-exec で以下を実施する。

1. `setsid()` により新セッションを作成し、制御 TTY から切り離す
2. `setsid()` が失敗した場合は、最低限 `setpgid(0,0)` で新しいプロセスグループを作成する

これにより、プロセスが実行中に TTY を触ろうとした場合でも、ジョブ制御シグナルによる stop（`STAT=T`）を避けやすくする。

## 実装詳細

- `src/process_manager.rs` にある `configure_process_group()` を単一の実装ポイントとして使用する。
  - 既に `setsid()` 優先 + `setpgid()` フォールバックを実装済みであるため、呼び出し側を統一する。
- `src/agent/runner.rs` の `build_command()` と `build_command_in_dir()` の Unix 側 `pre_exec` を、
  `configure_process_group(&mut cmd)` 呼び出しに置換する。

## 受け入れ条件

- `src/agent/runner.rs` から `setpgid()` を直接呼ばず、`configure_process_group()` を利用している。
- `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` が成功する。
