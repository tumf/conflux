## 1. TUI ResolveMerge ハンドラに on_merged フック実行を追加

- [x] 1.1 `src/tui/command_handlers.rs` の `ResolveMerge` ハンドラ（`Ok(_)` 分岐、現行 L632 付近）で、`ResolveCompleted` イベント送信前に `on_merged` フックを実行する。BranchMerge ハンドラ (L444-499) と同じパターンで: change_id 抽出 → HookRunner 構築 → task counts 取得 → `run_hook(OnMerged)` → `ResolveCompleted` 送信 (verification: `cargo build` が成功し、`rg "on_merged" src/tui/command_handlers.rs` で ResolveMerge 分岐内にフック実行コードが存在する)

## 2. テストと検証

- [x] 2.1 `cargo test test_on_merged_hook_execution` が引き続きパスすることを確認する (verification: テスト成功)
- [x] 2.2 `cargo clippy -- -D warnings` がパスすることを確認する (verification: clippy 成功)

## Future Work

- `resolve_deferred_merge()` 内の `ParallelExecutor` にも hooks を渡す設計の検討（現状は caller 側での実行で BranchMerge と一貫性がある）
