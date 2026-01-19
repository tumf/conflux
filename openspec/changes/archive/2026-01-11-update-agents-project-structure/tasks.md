## 1. AGENTS.md の Project Structure 更新

- [x] 1.1 現在の src/ ディレクトリ構造を反映
- [x] 1.2 存在しないモジュール（`opencode.rs`, `state.rs`）を削除
- [x] 1.3 新規モジュールを追加し説明を付与:
  - `analyzer.rs` - Change dependency analyzer for parallel execution
  - `config/` - Configuration file loading and management (mod.rs, defaults.rs, expand.rs, jsonc.rs)
  - `hooks.rs` - Lifecycle hook execution
  - `vcs/` - VCS backend trait abstraction (mod.rs, commands.rs, git/, jj/)
  - `history.rs` - Apply context history management
  - `task_parser.rs` - Native tasks.md parser
  - `parallel/` - Parallel execution (mod.rs, executor.rs, types.rs, events.rs, conflict.rs, cleanup.rs)
  - `parallel_run_service.rs` - Parallel execution service
  - `agent.rs` - AI agent command execution
  - `approval.rs` - Change approval management
  - `templates.rs` - Configuration templates
- [x] 1.4 tui/ サブディレクトリ構造を追加
- [x] 1.5 Key Dependencies 表に `async-trait` を追加
- [x] 1.6 State File Location セクションを Configuration Files セクションに更新
