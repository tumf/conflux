## Context
並列実行はcleanup guardのDropでworktree削除が走る。現状は「失敗時のみ保持」を前提に設計されているため、キャンセル/早期終了の経路で削除が発生し得る。

## Goals / Non-Goals
- Goals: worktreeは原則保持し、merged成功時のみ削除する挙動に統一する
- Non-Goals: worktreeの保存先や命名規則の変更、TUIの操作設計の変更

## Decisions
- Decision: cleanup guardは成功時以外はpreserve相当の扱いにし、Dropの削除は成功経路のみに限定する
- Alternatives considered:
  - 失敗/キャンセル時に設定フラグで保持を切り替える
  - 現状のままキャンセル経路のみ修正する

## Risks / Trade-offs
- 失敗/キャンセル時にworktreeが残り続けるため、ストレージ使用量が増える
- cleanupを明示的に実行する経路の網羅が必須になる

## Migration Plan
- 仕様更新後、cleanup guardとparallel executorの削除条件を変更する
- 成功時のみcleanupされることを確認する

## Open Questions
- なし
