## Context
並列実行で既存workspaceを再開する際、`Archive: <change_id>` がbaseブランチに存在すると `WorkspaceState::Merged` と判定される。実際には `openspec/changes/<change_id>` が残っている場合でもMerged扱いになり、apply/archiveが実行されずTUIが0%で停止する。

## Goals / Non-Goals
- Goals:
  - Merged判定を厳密化して不整合を防ぐ
  - Merged判定でスキップする場合でもTUIの状態を適切に更新する
- Non-Goals:
  - 既存のarchive/applyロジックの仕様変更
  - change の自動修復や強制削除

## Decisions
- Decision: `Archive: <change_id>` のコミット存在だけでMergedと判定せず、changesディレクトリの消失を追加条件とする
- Decision: Merged判定でスキップする場合は `MergeCompleted` イベントを送出し、TUIを `Merged` に更新する

## Risks / Trade-offs
- 変更検知にファイルシステム確認が追加されるため、状態判定に若干のI/Oコストが増える
- changesが手動で復活した場合はMerged判定されず再実行される可能性がある

## Migration Plan
1. Merged判定の条件追加
2. Mergedスキップ時のイベント送出
3. TUI表示の整合確認

## Open Questions
- なし
