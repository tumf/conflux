## Context
承認フローは approved ファイルを前提に CLI/TUI/Web 全体へ影響しており、実行判断の二重化と状態不整合の原因になっています。今回の変更は承認概念そのものを廃止し、起動時に全て未選択から開始する運用に切り替えます。

## Goals / Non-Goals
- Goals:
  - 承認概念の削除（approved ファイル・承認状態・@ 操作）
  - 起動時の実行マークを必ずクリア
  - 実行対象の判定を「選択/指定対象」のみに統一
- Non-Goals:
  - 既存の承認ファイルの移行や削除の自動化
  - 承認に代わる新しい承認/ロック機構の導入

## Decisions
- Decision: Change.is_approved を即時削除
  - 理由: 承認概念が不要になり、残すと死んだ分岐や誤解の温床になるため
- Decision: `approve` サブコマンドと Web 承認 API を削除
  - 理由: CLI/Web/TUI の UI/UX を整合させ、運用上の入口を完全に閉じるため
- Decision: `@` キーは no-op（キー操作から削除）
  - 理由: 承認機能廃止と一致させ、誤操作を防ぐため

## Risks / Trade-offs
- 既存の運用フロー（承認済みのみ実行）が利用できなくなる
  - 対応: 起動時は未選択で開始し、実行対象は明示的選択に限定する

## Migration Plan
- 既存の `openspec/changes/*/approved` は無視する
- 起動時に必ず未選択で開始する
- CLI/TUI/Web の承認関連 UI・API を同一変更で削除する

## Open Questions
- なし
