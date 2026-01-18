## Context
並列実行のanalysis対象がqueuedに限定される前提にもかかわらず、queued外のchangeがanalysis対象に残ることで、完了条件に到達できずループが継続する。

## Goals / Non-Goals
- Goals: queuedのみをanalysis対象に限定し、queuedと実行中が空なら終了する。
- Non-Goals: ワークスペース検出やarchive判定ロジックの変更、実行中changeの判定ルール変更。

## Decisions
- Decision: analysis前にqueued集合を算出し、queued以外を除外する。
- Alternatives considered: merged判定を基準に除外する案。queued基準の方が挙動が明確で実行状態と整合するため採用しない。

## Risks / Trade-offs
- queuedの定義がUI/CLIで異なる場合はズレが生じる可能性があるため、現行のqueue管理経路に合わせる必要がある。

## Migration Plan
- 既存の並列実行ループにフィルタを追加し、queuedと実行中が空の場合に終了判定を追加する。

## Open Questions
- なし
