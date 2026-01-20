## Context
TUI/Web UIのステータス語彙と表示形式は仕様で更新済みだが、実装側にprocessing/completedの旧語彙と旧表示形式が残っている。表示語彙とステータス遷移を整理し、`status:iteration` 形式を実装へ反映する必要がある。

## Goals / Non-Goals
- Goals:
  - TUI/Web UIの表示語彙を新語彙に統一する
  - apply/acceptance/archive/resolveのフェーズ表示に遷移する
  - `status:iteration` 表示に対応する
  - Web UIの集計指標を新語彙に合わせて更新する
- Non-Goals:
  - 既存のオーケストレーションロジックやイベント構造の再設計
  - ステータス語彙以外のUIデザイン変更

## Decisions
- Decision: 既存のQueueStatus enumに `Applying` を追加し、`Processing` を廃止する。
- Decision: 反復回数はQueueStatus表示レイヤーで `status:iteration` 形式に整形し、データモデル自体は既存の `iteration_number` を利用する。
- Decision: Web UI集計は `applying/accepting/archiving/resolving` を進行中扱いに統一する。

## Risks / Trade-offs
- 既存の `processing` 表示を使っている箇所を全て更新する必要があり、移行漏れがあると表示不整合が発生する。

## Migration Plan
1. TUIのQueueStatus表示語彙と遷移を更新する
2. TUIの表示を `status:iteration` 形式に変更する
3. Web UIの語彙・集計・表示を更新する
4. テストと手動確認で表示差異がないことを確認する

## Open Questions
- なし
