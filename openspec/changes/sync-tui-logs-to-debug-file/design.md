# Design: sync-tui-logs-to-debug-file

## アーキテクチャ概要

### 現状

```
OrchestratorEvent
    ↓
handle_orchestrator_event()
    ↓
add_log(LogEntry)  ←── TUI画面表示のみ
    ↓
AppState.logs (Vec<LogEntry>)
```

### 変更後

```
OrchestratorEvent
    ↓
handle_orchestrator_event()
    ↓
add_log(LogEntry)
    ├── AppState.logs (Vec<LogEntry>)  ←── TUI画面表示
    └── tracing::info/warn/error!()    ←── ファイル出力（--logs時）
```

## 設計詳細

### 1. LogEntryにログレベルを追加

現在`LogEntry`は`color`フィールドでレベルを暗黙的に表現している。明示的なログレベルを追加する。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warn,
    Error,
}

pub struct LogEntry {
    pub timestamp: String,
    pub message: String,
    pub color: Color,
    pub level: LogLevel,  // 新規追加
    pub change_id: Option<String>,
    pub operation: Option<String>,
    pub iteration: Option<u32>,
}
```

### 2. add_log()でtracing出力

`src/tui/state/logs.rs`の`add_log()`メソッドで、レベルに応じた`tracing`マクロを呼び出す。

```rust
impl AppState {
    pub fn add_log(&mut self, entry: LogEntry) {
        // tracing出力（--logs指定時のみ実際にファイルへ書き込まれる）
        match entry.level {
            LogLevel::Info | LogLevel::Success => {
                tracing::info!(target: "tui_log", "{}", entry.message);
            }
            LogLevel::Warn => {
                tracing::warn!(target: "tui_log", "{}", entry.message);
            }
            LogLevel::Error => {
                tracing::error!(target: "tui_log", "{}", entry.message);
            }
        }

        // 既存のTUI表示用処理
        self.logs.push(entry);
        // ... (以下省略)
    }
}
```

### 3. ログフォーマット

`target: "tui_log"`を指定することで、既存の`tracing::debug!()`によるログと区別できる。

出力例:
```
2025-01-16T21:18:11.123456+09:00 ERROR tui_log: Apply failed for add-global-resolve-lock: Agent command failed
```

## トレードオフ

### メリット

1. **一元化**: `add_log()`を通過するすべてのログがファイルにも出力される
2. **最小限の変更**: 1箇所の修正で全ログをカバー
3. **後方互換性**: `--logs`未指定時は動作に影響なし

### デメリット

1. **潜在的な重複**: 一部のログは既に`tracing::debug!()`で出力されている可能性がある
   - 対策: `target: "tui_log"`で区別可能
2. **ログ量の増加**: エージェント出力の各行がファイルに記録される
   - 対策: 既存の`log_deduplicator`と同様のサマリー機能を将来検討可能

## 代替案（採用しない）

### 案B: イベントハンドラで個別に追加

各イベントハンドラで`tracing::error!()`等を明示的に追加する方法。

- **不採用理由**: 
  - 追加漏れのリスクが高い
  - コードの重複が増える
  - 将来の新規イベント追加時に同様の対応が必要

## テスト戦略

1. **ユニットテスト**: `LogLevel`と`LogEntry`の各コンストラクタをテスト
2. **統合テスト**: `--logs`オプション付きでTUIを実行し、エラーイベント発生時にログファイルに出力されることを確認
