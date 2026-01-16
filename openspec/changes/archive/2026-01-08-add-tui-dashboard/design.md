# Design: TUIダッシュボード

## Context

OpenSpec Orchestratorは複数の変更を順次処理するCLIツールである。現在は `indicatif` で単純なプログレスバーを表示しているが、ユーザーは以下を要望している：
- 処理する変更を事前に選択したい
- 全体の進捗状況を一画面で把握したい
- 実行中に新しい変更が追加された場合に対応したい
- 動的にキューを管理したい

### 制約
- Rustで実装
- ターミナル環境で動作
- 非同期処理（tokio）との統合が必要

## Goals / Non-Goals

### Goals
- サブコマンドなしでTUIを起動
- 変更の選択機能（チェックボックス、キーボード操作）
- 選択した変更の進捗をダッシュボード表示
- ログ出力をリアルタイム表示
- 定期的な自動更新（5秒間隔）
- 新規変更の検出と視覚的表示
- 実行中の動的キュー追加

### Non-Goals
- マウス操作のサポート
- 設定ファイルによるカスタマイズ
- 実行中の変更のスキップ・中断機能
- 自動更新間隔のカスタマイズ（将来検討）

## Decisions

### TUIフレームワーク: ratatui + crossterm

**決定**: `ratatui` をTUIフレームワークとして、`crossterm` をバックエンドとして使用する。

**理由**:
- `ratatui` はRust TUIの事実上の標準（tui-rsのフォーク・後継）
- `crossterm` はクロスプラットフォーム対応
- 豊富なウィジェット（List、Gauge、Paragraph等）
- アクティブなメンテナンス

**代替案**:
- `cursive`: より高レベルだが、カスタマイズ性が低い
- `termion`: Linux/macOS専用、Windowsサポートなし

### モード設計: 選択モード → 実行モード

**決定**: TUIを2つのモードで構成する。実行モードでも選択操作が可能。

```
┌──────────────┐    F5     ┌──────────────┐
│  Select Mode │ ───────>  │  Running Mode │
│  (初期状態)   │           │  (処理実行)   │
└──────────────┘           └──────────────┘
       │                          │
       │  ↑↓ Space               │  ↑↓ Space (キュー追加)
       ▼                          ▼
    選択変更                   動的キュー追加
```

**選択モード**:
- 全変更をチェックボックス付きリストで表示
- デフォルトで全選択（既存変更）
- 新規変更はデフォルトで未選択
- ↑↓: カーソル移動
- Space: 選択トグル
- F5: 実行開始
- q: 終了

**実行モード**:
- 選択された変更のみ処理
- 進捗バー表示
- ログ表示
- ↑↓ Space: 未選択変更をキューに追加可能
- 完了後も表示維持（qで終了）

### 自動更新機能

**決定**: 5秒間隔で `openspec list` を実行し、変更一覧を更新する。

```
┌─────────────┐     5秒     ┌─────────────┐
│  AppState   │ <────────── │  Refresher  │
│  (changes)  │   update    │  (ticker)   │
└─────────────┘             └─────────────┘
```

**新規変更の検出ロジック**:
```rust
fn detect_new_changes(current: &[Change], fetched: &[Change]) -> Vec<Change> {
    fetched.iter()
        .filter(|f| !current.iter().any(|c| c.id == f.id))
        .cloned()
        .collect()
}
```

**新規変更の扱い**:
- `is_new: bool` フラグで管理
- `selected: false` （デフォルト未選択）
- 「NEW」バッジを表示
- 一定時間後（または次回更新後）にNEWフラグをクリア

### 動的実行キュー

**決定**: 実行中でもチェックボックス操作を許可し、選択された変更を次の処理サイクルでキューに追加する。

```
┌─────────────────────────────────────────────────┐
│  Execution Queue (FIFO)                         │
│  ┌─────┐ ┌─────┐ ┌─────┐                       │
│  │ A   │→│ B   │→│ C   │ ... (dynamically added)│
│  │(run)│ │(wait)│ │(wait)│                       │
│  └─────┘ └─────┘ └─────┘                       │
└─────────────────────────────────────────────────┘
```

**キュー状態**:
- `Processing`: 現在処理中
- `Queued`: キュー内で待機中
- `NotQueued`: 未選択（キュー外）

### アーキテクチャ: イベントループ分離型

**決定**: TUI描画ループ、オーケストレーション、自動更新を分離し、チャネル経由で状態を共有する。

```
┌─────────────────┐              ┌─────────────────┐
│   TUI Renderer  │ <── state ── │   App State     │
│   (main thread) │              │   (shared)      │
└────────┬────────┘              └────────┬────────┘
         │                                │
         │ events                         │ update
         ▼                                │
┌─────────────────┐    mpsc     ┌─────────────────┐
│  Event Handler  │ ─────────>  │   Orchestrator  │
│                 │  commands   │   (async task)  │
└─────────────────┘             └────────┬────────┘
                                         │
┌─────────────────┐    mpsc              │
│  Auto Refresher │ ─────────────────────┘
│  (5s ticker)    │   new changes
└─────────────────┘
```

**理由**:
- TUIは同期的な描画ループが必要
- オーケストレーションは非同期処理
- 自動更新は独立したタスクとして動作
- 疎結合により将来の拡張が容易

### レイアウト構成

**選択モード**:
```
┌─────────────────────────────────────────────────────────────┐
│  OpenSpec Orchestrator                        [Select Mode] │
│                                      Auto-refresh: 5s ↻     │
├─────────────────────────────────────────────────────────────┤
│  Changes (↑↓: move, Space: toggle, F5: run, q: quit)        │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ [x] ► add-env-openspec-cmd      3/6 tasks   50.0%     │ │
│  │ [x]   ignore-new-changes        0/7 tasks    0.0%     │ │
│  │ [ ]   new-feature           NEW 0/3 tasks    0.0%     │ │
│  └────────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│  Selected: 2 changes | New: 1                               │
│  Press F5 to start processing                               │
└─────────────────────────────────────────────────────────────┘
```

**実行モード**:
```
┌─────────────────────────────────────────────────────────────┐
│  OpenSpec Orchestrator                          [Running]   │
│                                      Auto-refresh: 5s ↻     │
├─────────────────────────────────────────────────────────────┤
│  Changes (Space: add to queue)                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ ► add-env-openspec-cmd      [████░░░░░░] 3/6   50.0%  │ │
│  │   ignore-new-changes        [queued]     0/7    0.0%  │ │
│  │ [ ] new-feature         NEW [not queued] 0/3    0.0%  │ │
│  └────────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│  Current: add-env-openspec-cmd                              │
│  Status: Applying via OpenCode...                           │
├─────────────────────────────────────────────────────────────┤
│  Logs                                                       │
│  2026-01-08 21:50:32 Starting apply for add-env-openspec... │
│  2026-01-08 21:51:00 Discovered new change: new-feature     │
└─────────────────────────────────────────────────────────────┘
```

### 状態管理

```rust
pub enum AppMode {
    Select,    // 変更選択モード
    Running,   // 実行中
    Completed, // 完了
}

pub struct AppState {
    pub mode: AppMode,
    pub changes: Vec<ChangeState>,
    pub cursor_index: usize,
    pub current_change: Option<String>,
    pub logs: Vec<LogEntry>,
    pub last_refresh: Instant,
    pub new_change_count: usize,
}

pub struct ChangeState {
    pub id: String,
    pub completed_tasks: u32,
    pub total_tasks: u32,
    pub queue_status: QueueStatus,
    pub selected: bool,
    pub is_new: bool,  // 新規検出フラグ
}

pub enum QueueStatus {
    NotQueued,    // 未選択
    Queued,       // キュー待機中
    Processing,   // 処理中
    Completed,    // 完了
    Archived,     // アーカイブ済み
    Error(String),
}
```

### CLI変更: デフォルトでTUI起動

**決定**: サブコマンドなしで起動した場合、TUIを表示する。

```bash
# TUIを起動（新しいデフォルト動作）
cflx

# 従来の動作（直接実行、TUIなし）
cflx run
cflx run --change <id>
```

**理由**:
- TUIが主要なユースケースになる
- 既存の `run` サブコマンドは後方互換性のため維持

## Risks / Trade-offs

| リスク | 影響 | 軽減策 |
|--------|------|--------|
| TUI描画がオーケストレーションをブロック | 処理遅延 | 別スレッドで描画、チャネル経由で通信 |
| 自動更新が頻繁すぎてリソース消費 | パフォーマンス低下 | 5秒間隔は妥当、将来設定可能に |
| ターミナルサイズが小さい場合のレイアウト崩れ | UX低下 | 最小サイズチェック、警告表示 |
| 依存関係の増加 | ビルド時間増加 | 許容範囲（ratatui + crossterm は軽量） |
| キーボード操作の学習コスト | UX低下 | ヘルプ行を常に表示 |
| 動的キュー追加時のレースコンディション | 処理重複 | 排他制御、キュー状態の厳密管理 |

## Migration Plan

1. `ratatui` と `crossterm` を依存関係に追加
2. `src/tui.rs` モジュールを新規作成（AppState、描画ロジック）
3. `src/cli.rs` を修正してデフォルトTUI起動を追加
4. 自動更新タスクを実装（5秒ticker）
5. 新規変更検出ロジックを実装
6. `Orchestrator` に動的キュー管理を追加
7. `main.rs` でモード分岐とTUIイベントループを実装
8. `progress.rs` は `run` サブコマンド用に当面維持

## Open Questions

- [x] デフォルト起動でTUIを表示 → 採用
- [x] 選択機能の操作方法 → ↑↓/Space/F5/q
- [x] 自動更新間隔 → 5秒
- [x] 新規変更のデフォルト状態 → 未選択
- [ ] 実行中に `q` で中断する機能は必要か？（現状はCtrl+Cのみ）
- [ ] ログの最大保持件数はいくつが適切か？（暫定100件）
- [ ] NEWバッジを消すタイミング（次回更新後？一定時間後？）
