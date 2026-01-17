## Context
worktreeが作成されたchangeではtasks.mdの最新版がworktree側にのみ存在するため、TUIはworktree側を唯一の参照元として扱う必要がある。Archived/Merged表示もworktree側のarchive済みtasks.mdから取得する。

## Goals / Non-Goals
- Goals:
  - worktree側のtasks.mdのみを進捗の参照元とする
  - Archived/Merged表示もworktree側のarchive済みtasks.mdから取得する
  - 未使用の読み取りコードがあれば削除する
- Non-Goals:
  - 並列実行のアーカイブ/マージの挙動を変更する
  - worktree生成方式やブランチ運用を変更する

## Decisions
- Decision: worktreeが存在するchangeではtasks.mdをworktree側からのみ取得する
- Decision: archiveディレクトリの解決はdate prefix付きにも対応する
- Alternatives considered: baseツリーのみで進捗を維持する

## Risks / Trade-offs
- worktree側tasks.mdが欠落している場合は進捗表示が失われる
- archiveディレクトリ探索の追加でauto-refreshの負荷が増える可能性

## Migration Plan
- 既存のrefresh処理をworktree優先（worktreeのみ参照）に調整
- 既存コードとの重複を整理し、影響のない範囲で削除

## Open Questions
- なし
