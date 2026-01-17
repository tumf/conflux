## Context
worktree のデフォルト作成先が一時ディレクトリ基準で統一されておらず、ユーザーからは `{root}/conflux/worktrees/{project}-{slug}/{change_id}` 形式の常設ディレクトリが期待されています。並列実行とTUIのworktree作成で同じ解決方法を使う必要があります。

## Goals / Non-Goals
- Goals: worktreeのデフォルト作成先を安定したデータディレクトリ配下へ統一する
- Goals: プロジェクト単位で衝突しないディレクトリ名を生成する
- Non-Goals: `workspace_base_dir` の明示設定挙動を変更しない
- Non-Goals: 既存worktreeの移行を自動化しない

## Decisions
- Decision: `workspace_base_dir` 未設定時のデフォルトは `{data_dir}/conflux/worktrees/{project_slug}` にする
- Decision: `{project_slug}` は `repo_basename` と `repo_root` 絶対パスの短いハッシュ（例: 8文字）で構成する
- Decision: `dirs::data_dir()` が取得できない場合のみ一時ディレクトリへフォールバックする
- Alternatives considered: repo_basename のみで構成 → 同名リポジトリ衝突の懸念があるため採用しない

## Risks / Trade-offs
- 既存worktreeが別パスに残る可能性があるが、明示設定がない場合のみ挙動を変えるため影響を限定する

## Migration Plan
1. 既存仕様と実装を更新し、新しいデフォルト解決を適用する
2. `workspace_base_dir` を設定している利用者は引き続き同一パスを使用する

## Open Questions
- 生成する `project_slug` のハッシュ長（8文字想定）に問題がないか
