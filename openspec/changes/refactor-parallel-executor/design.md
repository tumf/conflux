## Context

`parallel_executor.rs` は並列実行機能の中核だが、1580 行という規模になっている。
コード行数の内訳：

- `ParallelExecutor` 構造体とメソッド: ~1000 行
- `WorkspaceCleanupGuard`: ~100 行
- コンフリクト関連処理: ~200 行
- テスト: ~200 行

一つのファイルでこれらすべてを管理しているため、変更時の認知負荷が高い。

## Goals / Non-Goals

### Goals

- 各責務を独立したモジュールに分離し、可読性を向上
- モジュール単位でのテストを容易に
- 将来的な機能追加時の影響範囲を明確化

### Non-Goals

- 機能変更（リファクタリングのみ）
- パフォーマンス最適化

## Decisions

### ディレクトリ構造

```
src/parallel/
├── mod.rs              # ParallelExecutor (オーケストレーション層)
├── cleanup.rs          # WorkspaceCleanupGuard
├── conflict.rs         # detect_conflicts, resolve_conflicts_with_retry
├── events.rs           # ParallelEvent, send_event
├── executor.rs         # execute_apply_in_workspace, execute_archive_in_workspace
└── types.rs            # WorkspaceResult, 共通型
```

### モジュール責務

| モジュール | 責務 | 行数目安 |
|-----------|------|---------|
| `mod.rs` | グループ実行、マージ、高レベルオーケストレーション | 300-400 行 |
| `cleanup.rs` | ワークスペースのクリーンアップ保証 | 100 行 |
| `conflict.rs` | コンフリクト検出と解決ロジック | 200 行 |
| `events.rs` | イベント定義と送信ヘルパー | 100 行 |
| `executor.rs` | ワークスペース内での apply/archive 実行 | 300 行 |
| `types.rs` | 共通型定義 | 50 行 |

### 代替案と根拠

1. **そのまま維持** - 可読性が悪化し続けるため却下
2. **一部のみ分離** - 中途半端になるため却下
3. **完全分離（採用）** - 責務が明確で保守しやすい

## Risks / Trade-offs

- **リスク**: 分割によるインポート関係の複雑化
  - **緩和策**: `mod.rs` で必要な型を re-export
- **トレードオフ**: ファイル数増加 vs 個々のファイルの理解しやすさ

## Migration Plan

1. `src/parallel/` ディレクトリを作成
2. 型定義を `types.rs`, `events.rs` に移動
3. `cleanup.rs`, `conflict.rs`, `executor.rs` を順次作成
4. 残りのロジックを `mod.rs` に配置
5. 既存テストを各モジュールに移動
6. `parallel_executor.rs` を削除し、`mod.rs` から re-export

## Open Questions

- なし
