# Design: execution モジュールの基盤

## Context

OpenSpec Orchestrator は、serial mode と parallel mode の2つの実行モードをサポートしている。現在、これらのモードでは同じ目的のロジック（archive 処理、apply 処理、進捗チェックなど）が別々のファイルに実装されており、コードの重複と保守性の問題が発生している。

この変更は、重複解消のための基盤となる `src/execution/` モジュールを作成する。

## Goals / Non-Goals

### Goals
- Serial/Parallel mode で共通して使用可能な型と抽象化を提供
- 既存の `WorkspaceManager` や `HookRunner` との統合ポイントを確立
- 将来の共通ロジック抽出の基盤を作成

### Non-Goals
- 既存のコードの移行（後続の変更提案で実施）
- 新機能の追加
- 外部 API の変更

## Decisions

### Decision 1: モジュール構造

`src/execution/` ディレクトリを新設し、以下の構造とする：

```
src/execution/
├── mod.rs          # モジュールルート、re-exports
├── types.rs        # 共通型定義
├── archive.rs      # アーカイブ共通ロジック（後続変更で追加）
└── apply.rs        # Apply 共通ロジック（後続変更で追加）
```

**理由**: 関心の分離を維持しつつ、段階的な移行を可能にする

### Decision 2: ExecutionContext の設計

```rust
pub struct ExecutionContext<'a> {
    pub change_id: &'a str,
    pub workspace_path: Option<&'a Path>,  // None = main workspace
    pub config: &'a OrchestratorConfig,
    pub hooks: Option<&'a HookRunner>,
}
```

**理由**:
- `workspace_path` が `Option` なのは、serial mode ではメインワークスペースで動作するため
- `hooks` が `Option` なのは、parallel mode では段階的に hooks サポートを追加するため

### Alternatives considered

1. **既存モジュールに追加**: `orchestrator.rs` に共通ロジックを追加する案
   - 却下理由: ファイルが既に大きく、関心の分離が困難

2. **trait ベースの抽象化**: `Executor` trait を作成して serial/parallel を実装する案
   - 却下理由: 過度な抽象化。まずは関数ベースの共通化で十分

## Risks / Trade-offs

- **Risk**: モジュール追加による初期のビルド時間増加
  - Mitigation: 最小限の依存関係で開始し、必要に応じて追加

- **Trade-off**: 既存コードとの一時的な重複
  - 許容理由: 段階的な移行を優先し、一度に大きな変更を避ける

## Open Questions

- [ ] `ExecutionContext` に `VcsBackend` を含めるべきか、それとも別途渡すべきか
