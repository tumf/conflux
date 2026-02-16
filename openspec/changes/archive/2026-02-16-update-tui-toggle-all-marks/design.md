## Context
Changes ビューで実行マークの付け外しは Space の単体操作のみで、複数 change を一括で扱う手段がありません。TUI の既存操作体系に沿って、最小限のキー追加で全件トグルを実現します。

## Goals / Non-Goals
- Goals:
  - Changes ビューで全マーク/全アンマークを1キーで切り替える
  - 既存の queue 操作・実行停止・MergeWait/ResolveWait の制約を維持する
- Non-Goals:
  - Running/Stopping モード中の一括 stop/queue 操作
  - Worktrees ビューでの一括操作

## Decisions
- キーは `x` を採用する（`[x]` 表示と意味が一致し、既存割当と衝突しない）
- 対象は「実行マーク可能な change」に限定する
- 動作は「未マークが1つでもあれば全マーク、全件マーク済みなら全アンマーク」のトグル
- 適用モードは Select/Stopped に限定し、Running/Stopping/Error では無効化する

## Risks / Trade-offs
- Running 中に一括操作を許すと stop/add/remove が大量発火するため、適用範囲を限定して安全性を優先する

## Migration Plan
- 既存キーとの互換性を保ちながら `x` を追加するのみ

## Open Questions
- なし
