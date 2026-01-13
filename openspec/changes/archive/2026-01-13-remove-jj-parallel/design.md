## Context
現在の並列実行は jj workspace を優先し、Git worktree はフォールバックとして扱われています。運用現場では jj の導入が必須となり、利用開始のハードルが高い状況です。

## Goals / Non-Goals
- Goals: Git worktreeのみで並列実行を提供する
- Goals: CLI/TUI/設定のVCS選択をGit前提に揃える
- Non-Goals: 並列実行機能の削除や順次実行への回帰
- Non-Goals: Git worktree以外のVCSサポート追加

## Decisions
- Decision: jj backendとjj前提の自動判定を廃止し、並列実行はGit worktreeに統一する
- Decision: `--vcs` と `vcs_backend` の選択肢から `jj` を削除する
- Alternatives considered: jjとgitの両対応を維持しつつデフォルトのみ変更 → 仕様と運用が複雑なため不採用

## Risks / Trade-offs
- jj環境の既存ユーザーは並列実行の前提がGitに変わる
- 既存ドキュメントやテンプレートの記述更新が必要

## Migration Plan
- jj関連の設定値・CLIフラグを廃止する変更をリリースノートに明記する
- `.git` が存在しない場合のエラーメッセージをGit前提に統一する

## Open Questions
- なし
