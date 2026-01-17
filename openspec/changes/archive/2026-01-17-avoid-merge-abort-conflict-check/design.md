## Context
TUIのWorktree衝突チェックは現在git merge --no-commitとmerge --abortを使っている。この手法は作業ツリーを変更し、trackedなtasks.mdを復元するためアーカイブ処理が失敗する。

## Goals / Non-Goals
- Goals: 作業ツリーを汚さずに衝突チェックを行い、TUI起動中でもアーカイブに干渉しないようにする
- Non-Goals: 衝突検出のUI/表示ロジックの変更、Worktree更新頻度の変更

## Decisions
- Decision: git merge-tree --write-tree を用いて衝突判定を行い、merge --abort を廃止する
- Rationale: merge-treeは作業ツリーとindexを変更せずにマージ結果と衝突情報を取得できる

## Risks / Trade-offs
- git merge-treeの出力形式に依存するため、出力パースの厳密さが必要
- gitのバージョン差異で出力形式が変わる可能性がある

## Migration Plan
1. 衝突判定ロジックをmerge-treeベースへ切り替える
2. 既存のTUI衝突検出の結果が同等か確認する
3. 必要に応じてテストを更新する

## Open Questions
- git merge-treeの出力形式に依存しない最小限のパース基準をどう設計するか
