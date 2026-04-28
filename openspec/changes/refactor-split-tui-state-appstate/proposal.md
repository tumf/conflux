---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/tui/state.rs
  - src/tui/state/log_logic.rs
  - src/tui/state/selection_logic.rs
  - src/tui/state/worktree_logic.rs
  - src/tui/state/event_handlers/
  - openspec/specs/code-maintenance/spec.md
  - openspec/specs/tui-state-management/spec.md
---

# Change: TUI AppState の残存ロジック分割を完了する

**Change Type**: implementation

## Premise / Context

- `src/tui/state.rs` は約 4,000 行あり、既に `log_logic.rs`、`selection_logic.rs`、`worktree_logic.rs`、`event_handlers/` への分割が始まっているにもかかわらず、AppState の主要メソッド群がなお大量に残っている。
- たとえば `AppState::new()`、カーソル操作、resume / retry、ログ管理、refresh 補助など、異なる責務の実装が同一ファイルに集積している。
- 既存 canonical spec では code-maintenance に「TUI状態ロジックの責務分離後も公開挙動を維持する」が定義され、tui-state-management でも reducer 同期や queue semantics の維持が求められている。
- 現状は「一度分割を始めたが、巨大ファイルの入口と実装本体がまだ混在している」中間状態であり、今後の変更時に再び `state.rs` へ集中しやすい。

## Problem / Context

TUI 状態管理の構造は部分的にしか整理されておらず、選択・キュー・resume / retry・ログ・worktree などのロジックが `state.rs` に残り続けている。そのため、変更 1 件でも巨大ファイルの広い diff になりやすく、レビュー負荷・競合リスク・意図しない回帰の危険が高い。

特に queue/reducer 同期や error retry のような繊細な挙動は、構造変更時に regress しやすい一方、既存仕様では利用者から見える動作を保つことが最優先である。したがって本件は、先に characterization test で外形挙動を固定し、その後 AppState 実装を責務単位へさらに分ける形で進めるべきである。

## Proposed Solution

`src/tui/state.rs` を入口・型定義・再公開中心に寄せ、AppState の残存ロジックを責務別サブモジュールへ移す。

- 選択・キュー・resume / retry 系メソッドを責務別モジュールへ移す
- ログスクロール・パネル制御・補助表示系メソッドをログ責務へ寄せる
- worktree カーソルや action などのメソッドを worktree 責務へ寄せる
- reducer 同期・display status・公開 TuiCommand semantics は変えない
- Characterization test により selection / queue / retry / resume / log / worktree の既存挙動を先に固定する

## Acceptance Criteria

- selection / queue / retry / resume の公開挙動と shared reducer 同期はリファクタ前と同じである
- worktree カーソル操作と guard 判定の公開挙動は変わらない
- ログ追加・スクロール・panel toggle の公開挙動は変わらない
- `src/tui/state.rs` は入口・型定義・最小限の委譲中心になり、責務別実装がサブモジュールへ整理される
- API / CLI / TUI の利用者から見える操作意味論に変更がない

## Explicit Completion Conditions

- selection / queue / retry / resume / log / worktree の characterization test が追加または更新されている
- `state.rs` に残っていた AppState 実装のうち、少なくとも主要責務群が既存サブモジュールまたは新規責務モジュールへ移されている
- reducer 同期や display status の回帰がないことを既存または追加テストで確認できる
- `cargo test` と `cargo clippy --all-targets --all-features -- -D warnings` が成功する
- `cflx openspec validate refactor-split-tui-state-appstate --strict` が成功する

## Out of Scope

- TUI の表示デザインやキー割り当ての変更
- shared orchestration reducer の状態機械変更
- Web UI や server state への新機能追加
