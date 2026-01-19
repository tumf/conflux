## Context
order-based再分析ループではarchive完了後に個別mergeを実行しておらず、MergeWaitに遷移しないままworktreeがcleanupされる。既存のgroup実行パスではarchive完了後に`attempt_merge`を実行し、MergeDeferred時はMergeWaitとしてworktreeを保持している。

## Goals / Non-Goals
- Goals: order-based実行でもarchive後のmerge/resolveフローを既存仕様に一致させ、MergeDeferred時にworktreeを保持する
- Non-Goals: merge解決手順の変更やUI表示仕様の刷新

## Decisions
- order-basedループのarchive完了時に`attempt_merge`を実行し、結果に応じてMergeWait/cleanupを分岐する
- cleanupガードはMergeWait対象をpreserveし、Dropによる削除を防ぐ

## Risks / Trade-offs
- order-basedループにmerge処理が追加されるため、ログやイベントの発火が増えるが仕様整合性を優先する

## Open Questions
- なし
