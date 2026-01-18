## Context
TUI の MergeWait -> Resolving -> 完了時の状態遷移が、実際のマージ結果と一致しない。

## Goals / Non-Goals
- Goals: resolve 完了後に TUI が `Merged` を表示し、マージ済みの状態を正しく示す
- Non-Goals: merge/resolve の実行ロジックや Git 操作自体の変更

## Decisions
- Decision: resolve 完了時のイベントを `Merged` 表示へ一致させる
- Alternatives considered: `ResolveCompleted` は `Archived` のままにして別イベントを追加

## Risks / Trade-offs
- 既存の resolve 完了処理が `Archived` を前提としている可能性があるため、影響範囲を限定する

## Migration Plan
- TUI の状態遷移とテストを更新する

## Open Questions
- resolve 完了時に `MergeCompleted` を追加送信する設計が最も簡潔か
