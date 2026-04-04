# Design: TUI state 分割方針

## Context
`src/tui/state.rs` は単一ファイルに多数の責務を抱えており、変更の局所化が難しい。既存の `AppState` / `ChangeState` API を維持しながら、将来的に安全に分割できる中間形を定義する必要がある。

## Goals
- 公開挙動を変えずに内部責務を分離しやすくする
- テスト観点を責務境界に合わせて整理し、回帰検知を容易にする
- `runner` や `render` から見た呼び出し面はできるだけ維持する

## Proposed Boundaries
- 選択・カーソル移動
- 実行制御（start/resume/retry/resolve queue）
- ワークツリー操作
- ログ状態管理
- マージガードと検証ロジック

## Trade-offs
- 一度に完全分割すると呼び出し側変更が広がるため、まずは薄い内部モジュール化を優先する
- テストの配置変更は保守性向上につながるが、初回は既存テスト名や観点をなるべく維持して差分を小さくする

## Implementation Notes (this change)
- `state.rs` に責務別内部モジュールを追加し、選択可否判定 (`selection_logic.rs`)、ログバッファ/スクロール計算 (`log_logic.rs`)、ワークツリー削除ガード補助 (`worktree_logic.rs`) を抽出した
- `AppState` の公開メソッドシグネチャは変更せず、既存の `runner` / `event_handlers` からの呼び出し面を維持した
- 抽出した内部ロジックには単体テストを追加し、既存の `tui::state` テスト群と合わせて主要状態遷移の回帰を確認した

## Non-Goals
- TUIの見た目変更
- キーバインドやCLIオプション変更
- 並列実行アルゴリズム自体の仕様変更
