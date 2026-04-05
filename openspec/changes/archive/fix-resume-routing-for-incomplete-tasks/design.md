# Design: resumed implementation workspace の task incomplete は Apply を優先する

## Overview

implementation change では unchecked task が残る段階はまだ apply フェーズの継続が必要であり、acceptance は tasks 完了後の品質ゲートである。

v0.5.114 で確認された根本原因は、resume routing (`src/parallel/dispatch.rs` の `read_implementation_task_progress`) が `## Implementation Tasks` セクション限定で checkbox を数えるのに対し、archive guard (`src/task_parser.rs` の `parse_content`) がファイル全体の checkbox を数える、というスコープ不一致にある。`## Acceptance #N Failure Follow-up` 等の追加セクションに未完了 checkbox が残ると、resume routing は「完了」と判断し Acceptance に送るが、archive guard は「未完了」で拒否する。

## Goals

- tasks 未完了の resumed implementation workspace を Apply に戻す
- completed tasks change の既存 Acceptance > Archive routing は壊さない
- routing 理由を観測可能にする

## Non-Goals

- task parser format の変更
- acceptance/archive gate 内容の変更
- spec-only change の resume policy 追加

## Proposed Design

### Routing order

resumed implementation workspace では次の順で routing する:

1. unchecked implementation tasks remain -> Apply
2. implementation tasks complete and acceptance not durably passed -> Acceptance
3. implementation tasks complete and acceptance durably passed -> Archive

### Task completeness scope

resume routing の tasks 判定は archive guard と同じスコープを使う。具体的には `task_parser::parse_content` 相当のファイル全体 checkbox カウントを使い、`## Future Work` セクションのみを除外する（archive guard が `## Future Work` を除外している場合はそれに合わせる）。

`## Implementation Tasks` セクション限定の独自パーサー (`read_implementation_task_progress`) は廃止し、archive guard と同一の判定関数を呼ぶ。これにより routing と guard のスコープ不一致を構造的に防ぐ。

### Observability

Apply に戻したケースでは `tasks incomplete; rerouting resumed workspace to apply` 相当の理由をログまたはイベントへ残す。

## Test Strategy

1. `## Implementation Tasks` 完了だが `## Acceptance #N Failure Follow-up` に未完了 checkbox がある場合に Apply へ戻る回帰テスト（v0.5.114 再現ケース）
2. ファイル全体で tasks 完了の resumed workspace が Acceptance または Archive へ進む既存挙動維持テスト
3. `## Future Work` 項目が blocker にならないテスト
4. resume routing と archive guard の判定結果が一致することを確認するテスト
