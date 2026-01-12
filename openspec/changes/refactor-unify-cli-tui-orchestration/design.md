# Design: CLI/TUI オーケストレーション統合

## Context

現在の実装では、CLI モードと TUI モードで約2000行のオーケストレーションロジックが重複している。これは歴史的な経緯（TUI が後から追加された）による。

### 現在の問題
1. バグ修正が両方に適用されない
2. 機能追加時に2箇所を修正する必要
3. テストカバレッジの維持が困難
4. 機能の不一致が発生しやすい

## Goals / Non-Goals

### Goals
- 共通ロジックを1箇所に集約
- CLI/TUI の機能差を最小化
- テスト容易性の向上
- 段階的なリファクタリング（一度に全部やらない）

### Non-Goals
- CLI/TUI の完全統合（UI部分は別々のまま）
- 新機能の追加（リファクタリングのみ）
- パフォーマンス最適化

## Decisions

### アーキテクチャ

```
src/
├── orchestration/           # 新設：共通オーケストレーションロジック
│   ├── mod.rs
│   ├── archive.rs           # アーカイブ処理
│   ├── apply.rs             # Apply 処理
│   ├── selection.rs         # 変更選択ロジック
│   ├── state.rs             # 状態管理
│   └── hooks.rs             # フックコンテキスト構築ヘルパー
├── orchestrator.rs          # CLI エントリポイント（薄いラッパー）
└── tui/
    └── orchestrator.rs      # TUI エントリポイント（薄いラッパー）
```

### 共通インターフェース

```rust
/// オーケストレーション操作の結果
pub enum OperationResult {
    Success,
    Failed { error: String },
    Cancelled,
}

/// アーカイブ操作
pub async fn archive_change(
    change: &Change,
    agent: &mut AgentRunner,
    hooks: &HookRunner,
    context: &HookContext,
) -> Result<OperationResult>;

/// Apply 操作
pub async fn apply_change(
    change: &Change,
    agent: &mut AgentRunner,
    hooks: &HookRunner,
    context: &HookContext,
) -> Result<OperationResult>;

/// 変更選択（LLM分析オプション付き）
pub async fn select_next_change(
    changes: &[Change],
    agent: Option<&AgentRunner>,  // None の場合は進捗ベースのみ
) -> Result<Change>;
```

### ストリーミング対応

TUI はストリーミング出力が必要なため、コールバック方式を採用：

```rust
pub trait OutputHandler: Send + Sync {
    fn on_stdout(&self, line: &str);
    fn on_stderr(&self, line: &str);
}

// CLI: ログに出力
// TUI: イベントチャネルに送信
```

## Risks / Trade-offs

### リスク
1. **リグレッションリスク**: 大規模リファクタリングでバグ混入の可能性
   - 緩和: 段階的な移行、各フェーズでテスト確認

2. **API 変更**: 内部 API が変わるため、依存コードの修正が必要
   - 緩和: 内部 API のみなので外部影響なし

### トレードオフ
- 抽象化レイヤーの追加 → 若干のオーバーヘッド（許容範囲）
- 段階的移行 → 一時的に3箇所にコードが存在する期間あり

## Migration Plan

### Phase 1: 共通モジュール作成（最優先）
1. `src/orchestration/mod.rs` を作成
2. アーカイブ処理を共通化（パス検証バグ修正含む）
3. CLI/TUI から共通関数を呼び出すよう修正
4. テスト追加

### Phase 2: Apply 処理の統合
1. Apply 処理を共通化
2. ストリーミング対応の OutputHandler 導入
3. フック呼び出しを共通化

### Phase 3: 状態管理の統合
1. `OrchestratorState` 構造体を作成
2. CLI/TUI で使用

### Phase 4: 変更選択の統合（オプション）
1. LLM 分析を TUI でも使用可能に
2. 設定でオン/オフ切り替え

## Open Questions

1. TUI の `on_finish` フックは意図的に省略されている？
2. CLI の dry-run は TUI でも必要か？
3. LLM 分析は TUI でも常に有効にすべきか、設定可能にすべきか？
