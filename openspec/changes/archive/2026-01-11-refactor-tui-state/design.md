## Context

TUI の状態管理は `tui/state.rs` に集約されている。
ファイルサイズが大きく（1358 行）、以下の問題がある：

- `AppState` の責務が広すぎる（UI状態、ログ、イベント処理など）
- テストが 32 個あり、ファイルの半分近くを占める
- 変更時に全体を理解する必要がある

## Goals / Non-Goals

### Goals

- 状態管理の責務を明確に分離
- モジュール単位でのテスト容易性向上
- 新機能追加時の影響範囲を限定

### Non-Goals

- TUI の機能変更
- レンダリングロジックのリファクタリング（別提案）

## Decisions

### ディレクトリ構造

```
src/tui/state/
├── mod.rs              # AppState 本体と re-exports
├── change.rs           # ChangeState 構造体
├── modes.rs            # AppMode, モード切替ロジック
├── logs.rs             # ログエントリ管理、スクロール
└── events.rs           # handle_orchestrator_event
```

### 各モジュールの責務

| モジュール | 責務 | 主要な型・関数 |
|-----------|------|----------------|
| `mod.rs` | AppState 構造体定義と初期化 | `AppState::new()` |
| `change.rs` | 変更の状態表現 | `ChangeState`, `from_change()` |
| `modes.rs` | UI モードの管理 | `AppMode`, `start_processing()` |
| `logs.rs` | ログの追加・スクロール | `add_log()`, `scroll_logs_*()` |
| `events.rs` | オーケストレーターイベント処理 | `handle_orchestrator_event()` |

### 代替案と根拠

1. **そのまま維持** - ファイルが肥大化し続けるため却下
2. **AppState を複数の構造体に分割** - 既存 API 互換性の問題があるため却下
3. **メソッドのみ分離（採用）** - 外部 API を変えずに内部構造を改善

## Risks / Trade-offs

- **リスク**: `AppState` のフィールドへのアクセスが複雑化
  - **緩和策**: `impl AppState` ブロックを適切なモジュールに配置
- **トレードオフ**: ファイル数増加 vs 個々のファイルの可読性向上

## Migration Plan

1. `src/tui/state/` ディレクトリを作成
2. `ChangeState` を `change.rs` に移動
3. モード関連メソッドを `modes.rs` に移動
4. ログ関連メソッドを `logs.rs` に移動
5. イベント処理を `events.rs` に移動
6. 残りを `mod.rs` に配置
7. テストを各モジュールに分散
8. `tui/state.rs` を削除

## Open Questions

- なし
