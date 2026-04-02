## Context

現在の merge stall monitor は queue / scheduler の実進捗ではなく、base branch 上の直近 `Merge change:` コミット時刻だけを監視している。この信号は並列実行の liveliness を表さず、停止権限を持つと queue を壊し、停止権限を外すと継続実行での価値がほぼない。

最小修正により monitor は queue を停止できなくなったが、それにより存在意義自体がほぼ失われた。現状の monitor は有害または無用であり、削除するのが自然である。

## Goals / Non-Goals

- Goals:
  - 無用な merge stall monitor を完全に削除する
  - 並列実行から不要な監視コードと設定を取り除く
  - 設計を単純化し、将来本当に必要な health monitor を別設計で追加できる状態にする

- Non-Goals:
  - queue / scheduler の実進捗に基づく新しい health monitor の導入
  - stall policy の追加
  - serial 実行系の変更

## Decisions

- `MergeStallMonitor` モジュールを削除する
  - Alternatives: warn-only の observer として残す → 監視対象が wrong layer のままで、ノイズを増やすだけ
  - 選択理由: 観測対象が不適切なため、残す合理性がない

- `merge_stall_detection` 設定を削除する
  - Alternatives: 互換性のため残す → 実体のない死んだ設定になる
  - 選択理由: 不要な設定は保守負荷になる

## Risks / Trade-offs

- 既存ユーザーが `merge_stall_detection` を設定していても効果がなくなる → そもそも有効な価値を持たない設定だったため受容可能
- 本当に必要な stuck 検出は失われる → 現状の monitor は stuck 検出になっていないため、実質的な機能後退ではない

## Open Questions

- なし
