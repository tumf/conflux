## Context
parallel apply/archive の共通ループにおいて、hook 実行と ParallelEvent 発行の責務が複数箇所に分散しやすくなっています。イベント通知と実行タイミングのズレを防ぐため、共通ループ側で統一的に扱う設計が必要です。

## Goals / Non-Goals
- Goals: hook 実行と ParallelEvent 発行の統一ポイントを明確化する
- Goals: 現行の成功/失敗時の挙動を維持する
- Non-Goals: hook の種類追加、実行順序の変更、リトライ挙動の変更

## Decisions
- Decision: apply/archive 共通ループ内で hook 実行と ParallelEvent 発行をまとめて扱う
- Decision: hook 実行開始/完了/失敗のイベントを既存ルールのまま発行する
- Alternatives considered: hook 実行側にイベント発行を分散させる案は、変更箇所が増えるため採用しない

## Risks / Trade-offs
- 共通ループへ集約することで影響範囲が広がるため、挙動差分の確認が必要

## Migration Plan
- 既存の実装を共通ループに合わせて整理する
- hook とイベントの対応表をテスト観点として確認する

## Open Questions
- なし
