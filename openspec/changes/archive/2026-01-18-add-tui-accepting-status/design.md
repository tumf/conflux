## Context
TUI の status は `QueueStatus` とイベント更新で管理されている。acceptance の実行はログ出力のみで状態が区別されないため、実行フェーズが見えにくい。

## Goals / Non-Goals
- Goals: acceptance 実行中の明示的なステータス表示を追加する
- Non-Goals: acceptance 実行内容や判定ロジックの変更

## Decisions
- Decision: `QueueStatus` に `accepting` を追加し、acceptance 開始イベントで状態を更新する
- Alternatives considered: ログメッセージのみで識別（既存と同様で視認性が低い）

## Risks / Trade-offs
- ステータス追加に伴い表示の並びが増えるが、視認性向上を優先する

## Migration Plan
- 既存ステータスと互換性を保つため、新規バリアント追加のみで対応する

## Open Questions
- なし
