# configuration Specification

## Purpose
Defines the configuration file format, agent command templates, and settings for the orchestrator.
## Requirements
### Requirement: Environment Variable Configuration for OpenSpec Command

ユーザーは環境変数 `OPENSPEC_CMD` を通じて openspec コマンドを設定できなければならない (MUST)。

設定値の優先順位は以下の通りとする:
1. CLI 引数 `--openspec-cmd` (最優先)
2. 環境変数 `OPENSPEC_CMD`
3. デフォルト値 `npx @fission-ai/openspec@latest`

#### Scenario: 環境変数のみ設定

- **WHEN** 環境変数 `OPENSPEC_CMD` に `/usr/local/bin/openspec` が設定されている
- **AND** CLI 引数 `--openspec-cmd` が指定されていない
- **THEN** `/usr/local/bin/openspec` が openspec コマンドとして使用される

#### Scenario: CLI 引数が環境変数より優先

- **WHEN** 環境変数 `OPENSPEC_CMD` に `/usr/local/bin/openspec` が設定されている
- **AND** CLI 引数 `--openspec-cmd ./my-openspec` が指定されている
- **THEN** `./my-openspec` が openspec コマンドとして使用される

#### Scenario: どちらも未設定時はデフォルト値を使用

- **WHEN** 環境変数 `OPENSPEC_CMD` が設定されていない
- **AND** CLI 引数 `--openspec-cmd` が指定されていない
- **THEN** `npx @fission-ai/openspec@latest` が openspec コマンドとして使用される

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

- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "worktree_command": "opencode run --cwd {workspace_dir} '/openspec:proposal'"
  }
  ```
- **WHEN** ユーザーがTUIの `+` キーで提案作成フローを開始する
- **THEN** `worktree_command` が使用される

### Requirement: 設定ファイルの優先順位

オーケストレーターは以下の優先順位で設定ファイルを読み込まなければならない (MUST):

1. **プロジェクト設定** (優先): `.cflx.jsonc` (プロジェクトルート)
2. **グローバル設定**: `~/.config/cflx/config.jsonc`

プロジェクト設定が存在する場合はそちらを使用し、存在しない場合のみグローバル設定を使用する。

#### Scenario: プロジェクト設定がグローバル設定より優先される

- **GIVEN** グローバル設定 `~/.config/cflx/config.jsonc` に:
  ```jsonc
  { "apply_command": "global-agent apply {change_id}" }
  ```
- **AND** プロジェクト設定 `.cflx.jsonc` に:
  ```jsonc
  { "apply_command": "project-agent apply {change_id}" }
  ```
- **WHEN** 変更 `fix-bug` を適用する
- **THEN** `project-agent apply fix-bug` が実行される（プロジェクト設定が優先）

#### Scenario: プロジェクト設定がない場合はグローバル設定を使用

- **GIVEN** プロジェクトルートに `.cflx.jsonc` が存在しない
- **AND** グローバル設定 `~/.config/cflx/config.jsonc` に:
  ```jsonc
  { "apply_command": "global-agent apply {change_id}" }
  ```
- **WHEN** 変更 `fix-bug` を適用する
- **THEN** `global-agent apply fix-bug` が実行される

### Requirement: プレースホルダーの展開

コマンドテンプレート内のプレースホルダーは、実行時に実際の値に置換されなければならない (MUST)。

サポートするプレースホルダー:
- `{change_id}` - 変更ID（apply_command, archive_command で使用）
- `{prompt}` - システム提供の指示（apply_command, archive_command, analyze_command, resolve_command で使用）

#### Scenario: {change_id} プレースホルダーの正常な展開

- **WHEN** `apply_command` が `"agent run --apply {change_id}"` と設定されている
- **AND** 変更ID が `update-auth` である
- **THEN** 実行されるコマンドは `agent run --apply update-auth` となる

#### Scenario: 複数の {change_id} プレースホルダー

- **WHEN** `apply_command` が `"agent --id {change_id} --name {change_id}"` と設定されている
- **AND** 変更ID が `fix-bug` である
- **THEN** 実行されるコマンドは `agent --id fix-bug --name fix-bug` となる

#### Scenario: {prompt} プレースホルダーの展開

- **WHEN** `analyze_command` が `"claude '{prompt}'"` と設定されている
- **AND** 分析プロンプトが `"次に実行すべき変更を選択してください"` である
- **THEN** 実行されるコマンドは `claude '次に実行すべき変更を選択してください'` となる

#### Scenario: Both placeholders in apply command

- **WHEN** `apply_command` is `"agent --id {change_id} --instructions '{prompt}'"`
- **AND** change ID is `fix-bug`
- **AND** apply prompt is `"Focus on core changes"`
- **THEN** the executed command is `agent --id fix-bug --instructions 'Focus on core changes'`

#### Scenario: Multiple {prompt} placeholders

- **WHEN** `apply_command` is `"agent apply {change_id} --pre '{prompt}' --post '{prompt}'"`
- **AND** change ID is `fix-bug`
- **AND** apply prompt is `"Be careful"`
- **THEN** the executed command is `agent apply fix-bug --pre 'Be careful' --post 'Be careful'`

### Requirement: 依存関係分析コマンドの設定

`analyze_command` は LLM による依存関係分析に使用するコマンドを設定できなければならない (MUST)。このコマンドは `{prompt}` プレースホルダーを使用して分析プロンプトを受け取る。

#### Scenario: カスタム分析コマンドの使用

- **WHEN** `analyze_command` が `"claude-code '{prompt}'"` と設定されている
- **AND** 依存関係分析が実行される
- **THEN** `claude-code` にプロンプトが渡され、その出力が解析される

#### Scenario: 分析コマンド未設定時のフォールバック

- **WHEN** `analyze_command` が設定されていない
- **THEN** デフォルトの `opencode run --format json '{prompt}'` が使用される

### Requirement: JSONC 形式のサポート

設定ファイルは JSON with Comments (JSONC) 形式をサポートしなければならない (MUST)。

#### Scenario: コメント付き設定ファイルの解析

- **WHEN** 設定ファイルに以下の内容が含まれる:
  ```jsonc
  {
    // 適用コマンドの設定
    "apply_command": "codex run 'openspec-apply {change_id}'"
  }
  ```
- **THEN** コメントは無視され、設定が正常に読み込まれる

#### Scenario: 末尾カンマの許容

- **WHEN** 設定ファイルに末尾カンマが含まれる:
  ```jsonc
  {
    "apply_command": "codex run '{change_id}'",
  }
  ```
- **THEN** 末尾カンマは無視され、設定が正常に読み込まれる

### Requirement: フックコマンドの設定

オーケストレータは設定ファイルに `hooks` セクションを持ち、各段階に対応した任意コマンドを定義できなければならない (MUST)。

フックはすべてオプションであり、未設定のフックは実行されない。

#### Scenario: hooks 未設定

- **GIVEN** 設定ファイルに `hooks` セクションが存在しない
- **WHEN** オーケストレータを実行する
- **THEN** フックコマンドは一切実行されない

#### Scenario: 文字列（短縮形）でフックを設定

- **GIVEN** 設定ファイルに以下が存在する:
  ```jsonc
  {
    "hooks": {
      "on_start": "echo 'started'"
    }
  }
  ```
- **WHEN** オーケストレータを実行する
- **THEN** 開始時に `echo 'started'` が実行される

### Requirement: フック設定の詳細オプション

オーケストレータはフックごとに `continue_on_failure` と `timeout` を設定できなければならない (MUST)。

- `continue_on_failure` のデフォルト値は `true` とする
- `timeout` のデフォルト値は 60 秒とする

#### Scenario: continue_on_failure=false の場合はフック失敗で停止

- **GIVEN** `hooks.post_apply` が以下のように設定されている:
  ```jsonc
  {
    "hooks": {
      "post_apply": {
        "command": "exit 1",
        "continue_on_failure": false,
        "timeout": 60
      }
    }
  }
  ```
- **WHEN** post_apply が実行される
- **THEN** オーケストレータはエラーとして扱い処理を中断する

#### Scenario: timeout の超過

- **GIVEN** `hooks.on_start.timeout` が 1 秒に設定されている
- **AND** `hooks.on_start.command` がタイムアウトを超えて実行される
- **WHEN** `on_start` が実行される
- **THEN** フックはタイムアウトとして失敗扱いになる

### Requirement: フックのコンテキスト（プレースホルダーと環境変数）

オーケストレータはフック実行時に、コマンド文字列内のプレースホルダーを展開し、同等の情報を環境変数としても提供しなければならない (MUST)。

**Available placeholders and environment variables:**

| Placeholder | Environment Variable | Description |
|-------------|---------------------|-------------|
| `{change_id}` | `OPENSPEC_CHANGE_ID` | Current change ID |
| `{changes_processed}` | `OPENSPEC_CHANGES_PROCESSED` | Number of changes processed so far |
| `{total_changes}` | `OPENSPEC_TOTAL_CHANGES` | Total number of changes in initial queue |
| `{remaining_changes}` | `OPENSPEC_REMAINING_CHANGES` | Remaining changes in queue |
| `{completed_tasks}` | `OPENSPEC_COMPLETED_TASKS` | Completed tasks for current change |
| `{total_tasks}` | `OPENSPEC_TOTAL_TASKS` | Total tasks for current change |
| `{apply_count}` | `OPENSPEC_APPLY_COUNT` | Number of apply executions for current change |
| `{status}` | `OPENSPEC_STATUS` | Finish status (for on_finish: completed/iteration_limit/cancelled) |
| `{error}` | `OPENSPEC_ERROR` | Error message (for on_error hook) |
| N/A | `OPENSPEC_DRY_RUN` | Whether running in dry-run mode |

#### Scenario: change_id をプレースホルダーと環境変数で受け取る

- **GIVEN** `hooks.pre_apply.command` が `echo '{change_id} $OPENSPEC_CHANGE_ID'` に設定されている
- **WHEN** change `add-feature-x` に対して `pre_apply` が実行される
- **THEN** `{change_id}` は `add-feature-x` に展開される
- **AND** `OPENSPEC_CHANGE_ID` は `add-feature-x` として渡される

#### Scenario: apply_count でリトライ回数を追跡

- **GIVEN** `hooks.post_apply.command` が `echo 'Apply #{apply_count} for {change_id}'` に設定されている
- **WHEN** change `fix-bug` に対して2回目の `post_apply` が実行される
- **THEN** 出力は `Apply #2 for fix-bug` となる

### Requirement: Configuration Template Structure

Configuration templates generated by `init` command SHALL use a flat structure matching `OrchestratorConfig`.

#### Scenario: Claude template structure
- **WHEN** user runs `cflx init --template claude`
- **THEN** the generated config has `apply_command` at root level (not nested under `agent`)
- **AND** the generated config has `archive_command` at root level
- **AND** the generated config has `analyze_command` at root level
- **AND** the generated config has `hooks` at root level

#### Scenario: OpenCode template structure
- **WHEN** user runs `cflx init --template opencode`
- **THEN** the generated config has `apply_command` at root level
- **AND** the generated config has `archive_command` at root level
- **AND** the generated config has `analyze_command` at root level

#### Scenario: Codex template structure
- **WHEN** user runs `cflx init --template codex`
- **THEN** the generated config has `apply_command` at root level
- **AND** the generated config has `archive_command` at root level
- **AND** the generated config has `analyze_command` at root level

### Requirement: Claude Template Command Options

Claude template SHALL include verbose and streaming JSON output options for proper orchestrator integration.

#### Scenario: Claude apply command options
- **WHEN** Claude template is generated
- **THEN** `apply_command` includes `--verbose` flag
- **AND** `apply_command` includes `--output-format stream-json` flag
- **AND** `apply_command` uses `/openspec:apply {change_id}` prompt

#### Scenario: Claude archive command options
- **WHEN** Claude template is generated
- **THEN** `archive_command` includes `--verbose` flag
- **AND** `archive_command` includes `--output-format stream-json` flag
- **AND** `archive_command` uses `/openspec:archive {change_id}` prompt

#### Scenario: Claude analyze command options
- **WHEN** Claude template is generated
- **THEN** `analyze_command` includes `--verbose` flag
- **AND** `analyze_command` includes `--output-format stream-json` flag
- **AND** `analyze_command` uses `{prompt}` placeholder

### Requirement: System Prompt for Apply and Archive Commands

オーケストレーターは `apply_command` と `archive_command` の両方で `{prompt}` プレースホルダーをサポートし、システム提供の指示をエージェントコマンドへ注入できなければならない（SHALL）。

オーケストレーターは `apply_prompt` と `archive_prompt` の設定項目を提供し、ユーザーがカスタムのプロンプト値を定義できなければならない（SHALL）。

オーケストレーターは apply コマンド向けに、`apply_prompt` の直後に必ず付与されるハードコードシステムプロンプトを含めなければならない（SHALL）。このシステムプロンプトはタスク管理の必須ルールを強制し、ユーザー設定で無効化できない。

apply コマンドの `{prompt}` は `apply_prompt` + ハードコードシステムプロンプト + 履歴コンテキスト（存在する場合）を改行で連結したものとして展開されなければならない（SHALL）。

ハードコードシステムプロンプトには以下の指示が含まれなければならない（SHALL）。
- "Remove out-of-scope tasks."
- "Remove tasks that wait for or require user action."
- 未完了タスクは必ず実行可能であることを求める指示
- 実行不能タスクを具体コマンド + 合格基準を持つ実行可能タスクへ書き換える指示
- 人間判断や外部アクションが必須な場合のみ `(future work)` を付けて `Future work` セクションへ移動し、チェックボックスを外す指示
- apply が成功しても未完了タスクが残る状態を許容せず、タスク正規化を優先する指示

#### Scenario: Apply command prompt structure

- **GIVEN** `apply_command` が `"agent apply {change_id} {prompt}"` に設定されている
- **AND** `apply_prompt` が `"Focus on implementation."` に設定されている
- **WHEN** change `add-feature` を apply する
- **THEN** `{prompt}` は `"Focus on implementation.\n\nRemove out-of-scope tasks. Remove tasks that wait for or require user action."` に展開される
- **AND** 未完了タスクを実行可能に保つための指示が含まれる
- **AND** `(future work)` をチェックリストから外すための指示が含まれる

#### Scenario: Apply command with empty user prompt

- **GIVEN** `apply_command` が `"agent apply {change_id} {prompt}"` に設定されている
- **AND** `apply_prompt` が空、または未設定である
- **WHEN** change `add-feature` を apply する
- **THEN** `{prompt}` はハードコードシステムプロンプトのみを展開する

#### Scenario: Apply command with history context

- **GIVEN** `apply_command` が `"agent apply {change_id} {prompt}"` に設定されている
- **AND** `apply_prompt` が `"Focus on implementation."` に設定されている
- **AND** 以前の apply 失敗履歴が存在する
- **WHEN** change `add-feature` を apply する
- **THEN** `{prompt}` はユーザープロンプト + システムプロンプト + 履歴コンテキストに展開される

#### Scenario: Archive command unchanged

- **GIVEN** `archive_command` が `"agent archive {change_id} {prompt}"` に設定されている
- **AND** `archive_prompt` が `"Verify completion."` に設定されている
- **WHEN** change `add-feature` を archive する
- **THEN** `{prompt}` は `archive_prompt` のみに展開される（archive にはハードコードシステムプロンプトを追加しない）

### Requirement: Approved File Format

The approval system SHALL use a file-based approval mechanism with MD5 checksums.

#### Scenario: Approved file structure

- **WHEN** a change is approved
- **THEN** an `approved` file is created at `openspec/changes/{change_id}/approved`
- **AND** the file contains one line per tracked file
- **AND** each line format is `{md5sum}  {relative_path}` (two spaces between)
- **AND** paths are relative to project root

#### Scenario: Files included in approval

- **WHEN** generating the approved file
- **THEN** all `.md` files in the change directory are included
- **AND** files in subdirectories (e.g., `specs/cli/spec.md`) are included
- **AND** `tasks.md` is included in the manifest but excluded from validation
- **AND** files are sorted alphabetically by path

#### Scenario: Approved file example

```
47dadc8fb73c2d2ec6b19c0de0d71094  openspec/changes/my-change/design.md
88585d9f377f89cededbb4eeabcf9cf2  openspec/changes/my-change/proposal.md
c1fce89931c1142dd06f67a03059619d  openspec/changes/my-change/specs/cli/spec.md
ba74d36d6cdc1effcae37cfed4f97e19  openspec/changes/my-change/tasks.md
```

### Requirement: Approval Validation Logic

The system SHALL validate approval by checking for the presence of the `approved` file.

#### Scenario: Approved file exists

- **WHEN** checking approval status
- **AND** the `approved` file exists
- **THEN** the change is considered approved
- **AND** `is_approved` field is `true`

#### Scenario: Missing approved file means unapproved

- **WHEN** checking approval status
- **AND** the `approved` file does not exist
- **THEN** the change is considered unapproved
- **AND** `is_approved` field is `false`

### Requirement: Max Iterations Configuration

The orchestrator SHALL support a configurable maximum iteration limit to prevent infinite loops.

#### Scenario: Configure max iterations in config file

- **GIVEN** `.cflx.jsonc` contains:
  ```jsonc
  {
    "max_iterations": 100
  }
  ```
- **WHEN** the orchestrator runs
- **THEN** the loop stops after 100 iterations
- **AND** the finish status is `iteration_limit`
- **AND** a log message indicates "Max iterations (100) reached"

#### Scenario: Default limit when not configured

- **GIVEN** `max_iterations` is not set in config
- **WHEN** the orchestrator runs
- **THEN** the default limit of 50 iterations is applied
- **AND** the loop stops after 50 iterations if not complete

#### Scenario: CLI flag overrides config

- **GIVEN** config file has `"max_iterations": 100`
- **WHEN** user runs `cflx run --max-iterations 50`
- **THEN** the loop stops after 50 iterations
- **AND** CLI value takes precedence over config file

#### Scenario: Zero disables limit

- **GIVEN** `max_iterations` is set to `0`
- **WHEN** the orchestrator runs
- **THEN** no iteration limit is applied
- **AND** the loop continues until all changes complete or error occurs

#### Scenario: Warning when approaching limit

- **GIVEN** `max_iterations` is set to `100`
- **WHEN** iteration count reaches 80 (80% of limit)
- **THEN** a warning log is emitted: "Approaching max iterations: 80/100"

### Requirement: Iteration Limit Finish Status

The `on_finish` hook SHALL receive `iteration_limit` status when the loop stops due to reaching max iterations.

#### Scenario: Hook receives iteration_limit status

- **GIVEN** `max_iterations` is set to `10`
- **AND** `on_finish` hook is configured
- **WHEN** the loop reaches iteration 10
- **THEN** `on_finish` hook is called with `{status}` = `iteration_limit`
- **AND** `{iteration}` = `10`

### Requirement: Parallel Execution Configuration

The orchestrator SHALL support parallel execution configuration options in the config file. Parallel mode is OFF by default.

#### Scenario: Parallel mode disabled by default
- **WHEN** config file does not contain `"parallel_mode"` key
- **THEN** parallel execution mode is disabled
- **AND** CLI `--parallel` flag is required to enable it

#### Scenario: Enable parallel mode via config
- **WHEN** config file contains `"parallel_mode": true`
- **THEN** parallel execution mode is enabled by default
- **AND** CLI `--parallel` flag is not required
- **AND** git repository is required (`.git` directory must exist)

#### Scenario: Configure max concurrent workspaces
- **WHEN** config file contains `"max_concurrent_workspaces": 5`
- **THEN** at most 5 workspaces are created simultaneously
- **AND** CLI `--max-concurrent` overrides this value if provided

#### Scenario: Default max concurrent value
- **WHEN** `max_concurrent_workspaces` is not specified
- **THEN** the default value is 3

### Requirement: Workspace Base Directory Configuration

The orchestrator SHALL support configuring the base directory for git worktrees.

#### Scenario: Configure workspace directory
- **WHEN** config file contains `"workspace_base_dir": "/var/tmp/openspec-ws"`
- **THEN** worktrees are created under `/var/tmp/openspec-ws/`

#### Scenario: Default workspace directory
- **WHEN** `workspace_base_dir` is not specified
- **THEN** worktrees are created under system temp directory (e.g., `/tmp/openspec-workspaces/`)

### Requirement: Parallelization Analysis Prompt Configuration

The orchestrator SHALL support customizing the parallelization analysis prompt.

#### Scenario: Custom parallelization prompt
- **WHEN** config file contains `"parallelization_prompt": "custom prompt {changes}"`
- **THEN** the custom prompt is used for parallelization analysis
- **AND** `{changes}` is replaced with the list of pending changes

#### Scenario: Default parallelization prompt
- **WHEN** `parallelization_prompt` is not configured
- **THEN** a default prompt is used that asks the LLM to identify independent changes

### Requirement: Analyzer Dependency Output

The parallelization analyzer MUST return dependency information between changes to enable correct execution ordering.

#### Scenario: Analyzer returns dependency groups
- **WHEN** parallelization analysis is performed
- **THEN** the analyzer returns JSON with groups containing `depends_on` field:
  ```json
  {
    "groups": [
      {"id": 1, "changes": ["feature-a", "feature-b"], "depends_on": []},
      {"id": 2, "changes": ["integrate-ab"], "depends_on": [1]}
    ]
  }
  ```
- **AND** changes within the same group can run in parallel
- **AND** groups with `depends_on` wait for dependent groups to complete

#### Scenario: Circular dependency detection
- **WHEN** analyzer detects circular dependencies between changes
- **THEN** an error is returned with details about the circular dependency
- **AND** parallel execution is aborted

#### Scenario: Single change has no dependencies
- **WHEN** a change has no dependencies on other changes
- **THEN** the change is placed in a group with `depends_on: []`
- **AND** can run in parallel with other independent changes

#### Scenario: All changes are sequential
- **WHEN** analyzer determines all changes have dependencies
- **THEN** each change is placed in its own group
- **AND** `depends_on` forms a chain of sequential execution

### Requirement: Parallel Configuration in Templates

The `init` command templates SHALL include parallel execution configuration options.

#### Scenario: Claude template with parallel options
- **WHEN** user runs `cflx init --template claude`
- **THEN** the generated config includes commented parallel configuration:
  ```jsonc
  {
    // Parallel execution (requires git worktree)
    // "parallel_mode": false,
    // "max_concurrent_workspaces": 3
  }
  ```

### Requirement: VCS Backend Configuration

設定ファイルで VCS バックエンドを指定できなければならない（SHALL）。

#### Scenario: Configure VCS backend in config file

- **WHEN** `.cflx.jsonc` に以下が設定されている:
  ```jsonc
  {
    "vcs_backend": "git"
  }
  ```
- **AND** `--parallel` フラグで実行される
- **THEN** Git バックエンドが使用される

#### Scenario: VCS backend values

- **WHEN** `vcs_backend` を設定する
- **THEN** 有効な値は `"auto"`, `"git"` である
- **AND** デフォルト値は `"auto"` である

#### Scenario: CLI flag overrides config

- **WHEN** config ファイルに `"vcs_backend": "auto"` が設定されている
- **AND** `--vcs git` フラグが指定される
- **THEN** Git バックエンドが使用される（CLI が優先）

#### Scenario: Invalid VCS backend in config

- **WHEN** config ファイルに `"vcs_backend": "invalid"` が設定されている
- **THEN** 設定読み込み時にエラーが発生する
- **AND** エラーメッセージに有効な値が表示される

### Requirement: VCS Configuration in Templates

`init` コマンドで生成されるテンプレートに VCS 設定オプションを含めなければならない（SHALL）。

#### Scenario: Template includes VCS configuration

- **WHEN** `cflx init` が実行される
- **THEN** 生成される設定ファイルに以下のコメント付き設定が含まれる:
  ```jsonc
  {
    // VCS backend for parallel execution
    // "auto": detect automatically (git only)
    // "git": use git worktree
    // "vcs_backend": "auto"
  }
  ```

### Requirement: Web Monitoring Configuration

The configuration file SHALL support web monitoring settings to control HTTP server behavior.

#### Scenario: Enable web monitoring via config
- **WHEN** config file contains `web.enabled = true`
- **THEN** HTTP server starts automatically without `--web` CLI flag
- **AND** server uses configured port and bind address

#### Scenario: Configure web port in config file
- **WHEN** config file contains:
  ```jsonc
  {
    "web": {
      "enabled": true,
      "port": 9000
    }
  }
  ```
- **THEN** HTTP server binds to port 9000

#### Scenario: Configure bind address in config file
- **WHEN** config file contains:
  ```jsonc
  {
    "web": {
      "enabled": true,
      "bind": "0.0.0.0"
    }
  }
  ```
- **THEN** HTTP server accepts connections from any network interface

#### Scenario: CLI flags override config file
- **WHEN** config file has `web.port = 8080`
- **AND** user runs with `--web-port 3000` CLI flag
- **THEN** HTTP server binds to port 3000 (CLI takes precedence)

#### Scenario: Web disabled in config
- **WHEN** config file contains `web.enabled = false` or omits web section
- **THEN** HTTP server does not start unless `--web` CLI flag is provided

#### Scenario: Partial web configuration
- **WHEN** config file contains:
  ```jsonc
  {
    "web": {
      "port": 9000
    }
  }
  ```
- **AND** `enabled` field is omitted
- **THEN** web monitoring is disabled by default
- **AND** port setting is used only if `--web` CLI flag is provided

#### Scenario: Invalid port in config file
- **WHEN** config file contains `web.port = 99999` (out of valid range)
- **THEN** error message is displayed on startup
- **AND** orchestrator exits with non-zero status

#### Scenario: Default values when web enabled without specific settings
- **WHEN** config file contains only `web.enabled = true`
- **THEN** HTTP server uses default port 8080
- **AND** HTTP server uses default bind address 127.0.0.1

### Requirement: worktree_command のプレースホルダー展開

オーケストレーターは `worktree_command` のテンプレート内で以下のプレースホルダーを展開できなければならない（MUST）。

- `{workspace_dir}`: 作成した Git worktree の絶対パス
- `{repo_root}`: 元の Git リポジトリルート

展開される値は、既存のコマンドテンプレートと同様にシェル安全な形でエスケープされなければならない（MUST）。

#### Scenario: {workspace_dir} と {repo_root} が展開される

- **GIVEN** `worktree_command` が `"tool --repo {repo_root} --cwd {workspace_dir}"` に設定されている
- **WHEN** 生成されたworktreeのパスに空白が含まれる（例: `/tmp/my repo/ws-123`）
- **THEN** `{workspace_dir}` と `{repo_root}` はシェル安全に展開され、コマンドは意図した2つの引数として解釈される

### Requirement: Command Queue Configuration

オーケストレーターは JSONC 設定ファイルを通じてコマンド実行キューの動作を設定できなければならない (MUST)。

設定可能な項目は以下の通りとする：

1. `command_queue_stagger_delay_ms` - コマンド実行間の遅延時間（ミリ秒）、デフォルト: 2000
2. `command_queue_max_retries` - 自動リトライの最大回数、デフォルト: 2
3. `command_queue_retry_delay_ms` - リトライ間の待機時間（ミリ秒）、デフォルト: 5000
4. `command_queue_retry_patterns` - リトライ対象のエラーパターン（正規表現のリスト）
5. `command_queue_retry_if_duration_under_secs` - この秒数未満の実行時間で失敗した場合、リトライ対象とする、デフォルト: 5

デフォルトのリトライパターンは以下を含む：
- `Cannot find module` - モジュール解決エラー
- `ResolveMessage:` - モジュール解決メッセージ
- `EBADF.*lock` - ファイルロックエラー
- `Lock acquisition failed` - ロック取得失敗
- `ENOTFOUND registry\.npmjs\.org` - NPM レジストリ接続エラー
- `ETIMEDOUT.*registry` - レジストリタイムアウト

#### Scenario: デフォルト設定でキューが動作

- **WHEN** 設定ファイルにキュー設定が存在しない
- **THEN** デフォルト値（遅延2秒、最大2回リトライ、リトライ待機5秒）が使用される
- **AND** デフォルトのエラーパターンが適用される

#### Scenario: カスタム遅延時間の設定

- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_queue_stagger_delay_ms": 5000
  }
  ```
- **WHEN** コマンドが連続実行される
- **THEN** 各コマンド実行間に5秒の遅延が適用される

#### Scenario: カスタムリトライ設定

- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_queue_max_retries": 5,
    "command_queue_retry_delay_ms": 10000,
    "command_queue_retry_patterns": [
      "ECONNREFUSED",
      "timeout"
    ]
  }
  ```
- **WHEN** コマンド実行が `ECONNREFUSED` エラーで失敗
- **THEN** 最大5回まで自動リトライされる
- **AND** 各リトライ間に10秒の待機が発生する

#### Scenario: 空のリトライパターンリスト

- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_queue_retry_patterns": []
  }
  ```
- **WHEN** コマンド実行が任意のエラーで失敗
- **THEN** 自動リトライは実行されない（リトライパターンにマッチしないため）

#### Scenario: 遅延時間ゼロの設定

- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_queue_stagger_delay_ms": 0
  }
  ```
- **WHEN** コマンドが連続実行される
- **THEN** 遅延なしで即座に実行される（時間差起動が無効化）

#### Scenario: 実行時間による自動リトライ

- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "command_queue_retry_if_duration_under_secs": 5
  }
  ```
- **WHEN** コマンド実行が2秒で失敗
- **AND** エラーメッセージがリトライパターンにマッチしない
- **THEN** 実行時間が5秒未満のため、自動リトライされる

#### Scenario: 長時間実行後のエラーはリトライしない

- **GIVEN** `.cflx.jsonc` にデフォルト設定が使用される
- **WHEN** コマンド実行が30秒で失敗
- **AND** エラーメッセージがリトライパターンにマッチしない
- **THEN** 実行時間が5秒を超えているため、リトライされない

