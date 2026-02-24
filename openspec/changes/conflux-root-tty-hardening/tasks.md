---
title: タスク - Conflux 側の根本対策 (TTY停止/STAT=T 対策)
---

- [x] 1. 既存の実装点を確認し、TTY から切り離すための共通関数（`configure_process_group`）を利用する方針に確定する
- [x] 2. `src/agent/runner.rs` の `build_command()` を `configure_process_group()` 利用に切り替える
- [x] 3. `src/agent/runner.rs` の `build_command_in_dir()` を `configure_process_group()` 利用に切り替える
- [x] 4. 必要なテスト/コンパイルが通るように調整する
- [x] 5. `cargo fmt --check` / `cargo clippy -- -D warnings` / `cargo test` を実行して成功させる
