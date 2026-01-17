# Change: AI駆動コマンドの共通ランナー層を追加して開始遅延を共有する

## Why

現在、AI駆動コマンド（apply/archive/resolve/analyze）の実行経路が分散しており、`CommandQueue` による開始遅延（stagger）が一部の経路でしか効いていない。

具体的な問題点：
1. **並列 apply/archive が直接 spawn**: `src/parallel/executor.rs` で `Command::new("sh").spawn()` を直接呼んでいるため、`CommandQueue` を完全にバイパス
2. **resolve が毎回 AgentRunner を new**: `src/parallel/conflict.rs` で `AgentRunner::new()` を都度作成するため、Queue の `last_execution` 状態がリセットされ、stagger が実質無効
3. **stagger 状態が共有されない**: 各 `AgentRunner` が独自の `CommandQueue` を持つため、プロセス全体での開始遅延調整ができない

結果として、TUI で複数 change を queue すると AI エージェントが**ほぼ同時に起動**し、リソース競合（API レート制限、ファイルロック等）が発生しやすい。

## What Changes

- **共通ランナー層の導入**: AI駆動コマンド実行を一箇所に集約する `AiCommandRunner` モジュールを新設
- **stagger 状態の共有化**: `Arc<Mutex<Option<Instant>>>` をプロセス全体で共有し、すべてのAI駆動コマンドで開始遅延を適用
- **並列 apply/archive を共通ランナー経由に**: `src/parallel/executor.rs` の直接 spawn を排除
- **resolve の Queue 共有**: `AgentRunner::new()` を都度作らず、共有 runner を使用
- **analyze の出力検証強化**: exit 0 でも JSON が壊れていたらエラーとする strict validation を追加（コマンドランナー層で実施）

## Impact

- Affected specs: `command-queue`（「すべてのコマンド種別への適用」要件の実質的な実装）
- Affected code:
  - `src/ai_command_runner.rs`（新規）
  - `src/agent.rs`（共通ランナー層への委譲）
  - `src/parallel/executor.rs`（apply/archive の spawn 置換）
  - `src/parallel/conflict.rs`（resolve の AgentRunner 共有）
  - `src/parallel/mod.rs`（共有 runner の初期化）
  - `src/analyzer.rs`（strict JSON validation）
