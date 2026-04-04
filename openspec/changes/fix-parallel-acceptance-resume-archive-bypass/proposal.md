---
change_type: implementation
priority: high
dependencies: []
references:
  - src/parallel/dispatch.rs
  - src/parallel/executor.rs
  - src/execution/state.rs
  - openspec/specs/parallel-execution/spec.md
  - ~/.local/state/cflx/logs/conflux-bda270b8/2026-04-04.log
---

# Change: parallel再開時のacceptance未完了archive遷移を防止する

**Change Type**: implementation

## Premise / Context

- ユーザー報告では、parallel モードで tasks が 100% の change を cflx 再起動後に再開すると acceptance を飛ばして archive から始まることがある。
- 実ログ `~/.local/state/cflx/logs/conflux-bda270b8/2026-04-04.log` では `split-rejecting-from-acceptance` が acceptance cycle 5 を開始した後、`ACCEPTANCE: PASS` / `ACCEPTANCE: FAIL` / `Acceptance passed for ...` を出さずに archive を開始している。
- 現行 spec `openspec/specs/parallel-execution/spec.md` は、archive 未完了の resumed workspace では acceptance を再実行してから archive に進むことを要求している。
- `src/parallel/dispatch.rs` は acceptance 結果を durable に保持しておらず、resume 時の archive 遷移条件が acceptance 完了保証より弱い。

## Problem / Context

parallel 実行では archive 開始前に acceptance を必須ゲートとして再実行する仕様になっているが、実際には acceptance 実行中断や再起動を挟むと、acceptance の最終 verdict が確認できていない workspace が archive に入ることがある。

この不整合により、品質ゲート未通過の change が archive されうる。特に acceptance FAIL の follow-up を抱えた workspace でも、再開経路によって acceptance 完了確認なしに archive command が起動するため、parallel resume の安全性が失われる。

## Proposed Solution

parallel resume に「archive 開始には durable な acceptance-pass 証跡が必要」という明示的な境界を追加する。

具体的には:

1. workspace 内に acceptance の durable state（少なくとも pending / running / passed / failed-or-interrupted を区別できる状態）を保持する。
2. resumed workspace が `Applied` または `Archiving` と判定されても、最新 acceptance state が `passed` でなければ archive を開始しない。
3. acceptance 実行開始時に running 状態を記録し、verdict なしの中断・再起動後は archive ではなく acceptance 再実行に戻す。
4. archive 開始直前に durable acceptance-pass state を再検証し、欠けていれば archive command を起動しない。
5. TUI / log / tests で「acceptance 未完了のため archive を抑止して acceptance を再実行した」ことを観測可能にする。

## Acceptance Criteria

- parallel resumed workspace は、archive 未完了である限り、durable acceptance-pass state がないまま archive を開始しない。
- acceptance 実行開始後に cflx が停止または再起動しても、その workspace は次回再開時に archive へ進まず acceptance 再実行に戻る。
- acceptance FAIL / BLOCKED / verdict 未確定の各ケースで archive command は起動しない。
- parallel resume routing と archive 入口ガードの両方に回帰テストが追加される。
- TUI / tracing ログから「resume 時に acceptance が未完了だったため archive を抑止した」ことが確認できる。

## Out of Scope

- serial モードの scheduler / archive 条件の再設計
- acceptance prompt 内容や agent 文面の改善
- archive quality gate 自体の内容変更
