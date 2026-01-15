# Change: Git worktree 作成時にブランチも作成する（parallel / TUI `+`）

## なぜ（Why）
Git worktree を **detached HEAD** で作成すると、作業内容をコミットしてブランチとして管理・共有（push / PR）する手順が分かりづらく、また「この worktree がどのブランチに紐づくか」が追跡しにくい。

現状、
- parallel 実行（`--parallel`）では change ごとの worktree が作成される
- TUI の `+` キーによる提案作成フローでも一時ディレクトリ配下に worktree が作成される

が、後者（`+` フロー）は detached worktree のため、提案作成を開始した直後からブランチが存在しない。

## 何を変えるか（What Changes）
- Git worktree を作成する際は、用途（parallel / `+`）を問わず **必ずブランチを作成して worktree をそのブランチに紐づける**。
- parallel 実行の worktree ブランチ名は **`{change_id}`** とする。
- TUI の `+` フローで作成される worktree ブランチ名は **`oso-session-<rand>`** とする。
- resume は「安全に一致判定できる場合のみ」行い、不整合があれば既存 worktree/ブランチを自動削除して作り直す。

## 影響範囲（Impact）
- 影響する仕様:
  - `parallel-execution`
  - `tui-propose-input`
- 関連する実装領域（参考）:
  - Git worktree 作成（`git worktree add -b ...`）
  - TUI の `+` キー起点の worktree 作成

## 非ゴール（Non-Goals）
- ブランチ命名規則のユーザー設定化（config 化）
- 既存の worktree 自動クリーンアップ戦略の変更
- jj workspaces の挙動変更

## 受け入れ条件（Acceptance Criteria）
- parallel 実行で作成される各 worktree は detached ではなくブランチに紐づいており、ブランチ名は `{change_id}` である。
- TUI の `+` で作成される worktree は detached ではなくブランチに紐づいており、ブランチ名は `oso-session-<rand>` である。
- resume は安全に一致判定できる場合のみ行われ、不整合があれば既存 worktree/ブランチを削除して新規作成される。
- どちらのフローでも、作成直後の worktree で `git status` が通常通り動作し、ブランチが確認できる。
