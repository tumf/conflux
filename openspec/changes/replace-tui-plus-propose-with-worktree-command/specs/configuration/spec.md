## MODIFIED Requirements

### Requirement: エージェントコマンドの設定ファイル

オーケストレーターは JSONC 形式の設定ファイルを通じてエージェントコマンドを設定できなければならない (MUST)。

設定可能なコマンドは以下の種類とする:

1. `apply_command` - 変更の適用コマンド
2. `archive_command` - 変更のアーカイブコマンド
3. `analyze_command` - 依存関係分析コマンド
4. `resolve_command` - Git マージの完了（merge/add/commit）や競合解消に使用するコマンド
5. `hooks` - 段階フックコマンド
6. `propose_command` - （後方互換のため残り得る）提案作成コマンド
7. `worktree_command` - TUIの `+` から起動される worktree 上の提案作成コマンド

#### Scenario: worktree_command を設定できる

- **GIVEN** `.openspec-orchestrator.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "worktree_command": "opencode run --cwd {workspace_dir} '/openspec:proposal'"
  }
  ```
- **WHEN** ユーザーがTUIの `+` キーで提案作成フローを開始する
- **THEN** `worktree_command` が使用される

## ADDED Requirements

### Requirement: worktree_command のプレースホルダー展開

オーケストレーターは `worktree_command` のテンプレート内で以下のプレースホルダーを展開できなければならない（MUST）。

- `{workspace_dir}`: 作成した Git worktree の絶対パス
- `{repo_root}`: 元の Git リポジトリルート

展開される値は、既存のコマンドテンプレートと同様にシェル安全な形でエスケープされなければならない（MUST）。

#### Scenario: {workspace_dir} と {repo_root} が展開される

- **GIVEN** `worktree_command` が `"tool --repo {repo_root} --cwd {workspace_dir}"` に設定されている
- **WHEN** 生成されたworktreeのパスに空白が含まれる（例: `/tmp/my repo/ws-123`）
- **THEN** `{workspace_dir}` と `{repo_root}` はシェル安全に展開され、コマンドは意図した2つの引数として解釈される
