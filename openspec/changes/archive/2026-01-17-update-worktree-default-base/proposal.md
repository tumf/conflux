# Change: Worktreeのデフォルト作成先をConfluxデータディレクトリ配下へ統一

## Why
現状のworktree作成先が一時ディレクトリ基準のため、場所が分かりにくく消える可能性があります。プロジェクトごとに一貫した場所へ統一することで、利用者がworktreeを追跡しやすくなります。

## What Changes
- `workspace_base_dir` 未設定時のデフォルトを `{data_dir}/conflux/worktrees/{project_slug}` に変更する
- `workspace_base_dir` 設定時は従来通りその値を使用する
- `project_slug` をリポジトリ名と絶対パスの短いハッシュで構成する
- 並列実行（worktree）とTUIのworktree作成で同一の解決ルールを使う
- 仕様上のデフォルトパス記述を更新する

## Impact
- Affected specs: configuration, parallel-execution, tui-worktree-view
- Affected code: config, parallel execution, TUI worktree creation
