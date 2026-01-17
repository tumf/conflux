## Context

AI駆動コマンド（`apply_command`, `archive_command`, `resolve_command`, `analyze_command`）は、すべて外部AIエージェント（opencode run 等）を起動するコマンドである。これらのコマンドは同時起動するとリソース競合（API レート制限、ファイルロック、モジュール解決失敗等）が発生しやすい。

現在の `CommandQueue` は stagger（開始遅延）と retry のメカニズムを提供しているが、以下の理由で十分に機能していない：

1. `AgentRunner` が内部で独自の `CommandQueue` を持つため、インスタンス間で `last_execution` が共有されない
2. 並列実行モードの apply/archive は `AgentRunner` を経由せず、直接 `Command::new("sh").spawn()` を呼んでいる
3. resolve は `AgentRunner::new()` を毎回作成するため、Queue 状態がリセットされる

## Goals / Non-Goals

### Goals
- すべてのAI駆動コマンドで開始遅延（stagger）を共有する
- コマンド実行の散逸を防ぎ、一箇所で管理する
- analyze の出力検証を厳格化し、LLM のブレによる無効応答を検出する

### Non-Goals
- Git コマンドや内部ユーティリティコマンドへの stagger 適用（AI駆動系のみ対象）
- retry ロジックの変更（既存の `CommandQueue` ロジックを維持）
- 完全なシリアライズ実行（開始だけずらし、並列性は維持）

## Decisions

### Decision 1: 2層アーキテクチャの採用

**共通ランナー層**と**コマンドランナー層**に分離する。

```
┌─────────────────────────────────────────────────────┐
│  コマンドランナー層 (AgentRunner)                    │
│  - apply/archive/resolve/analyze の public API      │
│  - コマンドごとの出力検証（analyze: JSON parse 等）  │
│  - プロンプト構築・履歴管理                          │
└─────────────────────────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────┐
│  共通ランナー層 (AiCommandRunner)                    │
│  - spawn 前の stagger 適用                           │
│  - stdout/stderr の streaming                        │
│  - retry ロジック（CommandQueue 経由）              │
│  - 共有状態: Arc<Mutex<Option<Instant>>>            │
└─────────────────────────────────────────────────────┘
```

**理由**: 「実行の機械部分」と「コマンドの意味解釈」を分離することで、各責務が明確になり、テスト・保守が容易になる。

### Decision 2: stagger 状態の共有方法

`Arc<Mutex<Option<Instant>>>` を上位（Orchestrator / ParallelExecutor / TUI）で1つ作成し、すべての `AiCommandRunner` インスタンスに注入する。

```rust
// 初期化（main.rs / tui/runner.rs）
let shared_last_execution = Arc::new(Mutex::new(None));

// ParallelExecutor に渡す
let executor = ParallelExecutor::new(config, shared_last_execution.clone());

// AgentRunner にも渡す（serial mode）
let runner = AgentRunner::new_with_shared_state(config, shared_last_execution.clone());
```

**代替案**: `CommandQueue` 自体を `Arc<CommandQueue>` で共有する方法も検討したが、`CommandQueue` は他の設定（retry patterns 等）も含むため、必要最小限の `last_execution` のみ共有する方が単純。

### Decision 3: 並列 apply/archive の実装変更

`src/parallel/executor.rs` の `Command::new("sh").spawn()` を、共通ランナー経由に置き換える。

**Before:**
```rust
let mut child = Command::new("sh")
    .arg("-c")
    .arg(&command)
    .spawn()?;
```

**After:**
```rust
let (child, rx) = shared_runner
    .execute_streaming(&command, Some(workspace_path))
    .await?;
```

### Decision 4: analyze の strict JSON validation

`analyze_dependencies()` の返り値を `Result<AnalysisResult>` に変更し、以下をエラーとする：

1. exit code が 0 以外
2. stdout が JSON としてパースできない
3. 必須キー `groups` が存在しない
4. `groups` が配列でない

**期待 JSON 形式**（`src/analyzer.rs` のプロンプトで定義済み）:
```json
{
  "groups": [
    { "id": 1, "changes": ["change-a"], "depends_on": [] }
  ]
}
```

## Risks / Trade-offs

### Risk: stagger による全体実行時間の増加
- **Mitigation**: デフォルト stagger delay は 2秒のまま維持。並列性自体は維持されるため、影響は限定的。

### Risk: 既存テストへの影響
- **Mitigation**: `AgentRunner::new()` は既存のまま維持し、`new_with_shared_state()` を追加。既存テストは変更不要。

### Trade-off: 共有状態の導入による複雑化
- **Acceptance**: `Arc<Mutex<...>>` は Rust の標準的なパターンであり、既存の `global_merge_lock()` と同様のアプローチ。許容範囲。

## Migration Plan

1. `AiCommandRunner` モジュールを追加（既存コードに影響なし）
2. `AgentRunner::new_with_shared_state()` を追加（既存 `new()` は維持）
3. 並列 executor を共通ランナー経由に段階的に移行
4. resolve の AgentRunner 共有を実装
5. analyze の strict validation を追加

**Rollback**: 各ステップは独立しており、問題発生時は該当コミットを revert 可能。

## Open Questions

なし（設計決定済み）
