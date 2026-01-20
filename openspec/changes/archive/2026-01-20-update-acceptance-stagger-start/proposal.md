# Change: 並列 acceptance の開始をスタッガー共有に統一

## Why
並列実行時に acceptance コマンドが同時起動し、スタッガーが効かずエージェント起動エラーが増えるため。

## What Changes
- 並列実行で作成される `AgentRunner` が共有スタッガー状態を使うようにする。
- `acceptance_command` の起動が設定された遅延に従うことを保証する。

## Impact
- Affected specs: command-queue
- Affected code: `src/parallel/mod.rs`, `src/parallel/executor.rs`, `src/parallel/conflict.rs`, `src/agent/runner.rs`

## 実装完了（第8回修正）

### 最終的な達成
すべての AI エージェントコマンド（apply, archive, acceptance, resolve, analyze, worktree）が、CLI/TUI の両モード、直列/並列実行の両方で、プロセス全体で単一の `SharedStaggerState` を共有するようになりました。また、将来使用される共通オーケストレーションコード（`src/execution/apply.rs`, `src/orchestration/apply.rs`）も `AiCommandRunner` を使用するように修正されました。

### 主要な変更

**1. 並列実行の共有状態統一（第7回までに完了）**
- `src/parallel/mod.rs`: `with_backend_and_queue_and_stagger()` コンストラクタを追加し、外部から `shared_stagger_state` を受け取れるようにした
- `src/parallel_run_service.rs`: `ParallelExecutor` 作成時に自身の `shared_stagger_state` を渡すように変更
- **第7回修正**: `create_executor_with_queue_state()` メソッドが `with_backend_and_queue_and_stagger()` を使用し、`shared_stagger_state` を確実に渡すように修正
- これにより、`ParallelRunService` 内の analyze コマンドと `ParallelExecutor` 内の apply/archive/acceptance が同じスタッガー状態を共有

**2. TUI の共有状態統一（第7回までに完了）**
- `src/tui/runner.rs`: TUI 起動時に1つの `shared_stagger_state` を作成
- `src/tui/orchestrator.rs`: `run_orchestrator()` と `run_orchestrator_parallel()` に `shared_stagger_state` パラメータを追加
- これにより、worktree_command と apply/archive/acceptance が同じスタッガー状態を共有

**3. 共通オーケストレーションコードの修正（第8回）**
- `src/execution/apply.rs:execute_apply_loop()` - `ai_runner` パラメータを追加し、`run_apply_streaming_with_runner()` を使用
- `src/orchestration/apply.rs:apply_change_streaming()` - `ai_runner` パラメータを追加し、`run_apply_streaming_with_runner()` を使用
- `src/parallel/orchestration_adapter.rs:apply_change_in_workspace()` - `ai_runner` パラメータを追加
- これらの関数は現在 `#[allow(dead_code)]` でマークされていますが、将来的にリファクタリングで使用される際に確実に `AiCommandRunner` を経由するようになりました

### 検証結果
- ✅ 全863テストがパス (833 unit + 25 e2e + 2 merge conflict + 3 process cleanup)
- ✅ `cargo fmt --check` - フォーマット正常
- ✅ `cargo clippy -- -D warnings` - 警告なし

### 効果
設定されたスタッガー遅延（デフォルト2000ms）がすべての AI エージェントコマンドに統一的に適用され、同時起動による初期化エラーとリソース競合を完全に防止します。現在のコードパスだけでなく、将来的に使用される共通オーケストレーションコードも含めて、すべての apply コマンド実行が `AiCommandRunner` を経由するようになりました。
