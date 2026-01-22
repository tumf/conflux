## Context

Running ヘッダーのカウントが queued を含むため実際の稼働数と一致せず、さらに手動 resolve が並列スロット外で実行されることで dispatch が早期に進む問題がある。

## Goals / Non-Goals

- Goals:
  - Running ヘッダーのカウント対象を in-flight 状態に限定する
  - 手動 resolve を in-flight として扱い、スロット計算に反映する
- Non-Goals:
  - resolve 実行の UX やコマンドフローの変更
  - 既存の自動 resolve の挙動変更

## Decisions

- Decision: `QueueStatus::is_active` を in-flight 定義に合わせ、queued を除外する
- Decision: 手動 resolve の開始/終了を共有状態に反映し、parallel スケジューラの `available_slots` 計算に含める

## Risks / Trade-offs

- in-flight 定義が拡張されるため、手動 resolve 中は dispatch が一時停止する
- 実行スロット消費の見た目と実際の同時実行数を一致させることを優先する

## Migration Plan

1. `QueueStatus::is_active` と Running 表示の更新
2. 手動 resolve の in-flight 反映とテスト追加

## Open Questions

- なし
