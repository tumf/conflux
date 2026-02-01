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

`apply_command`/`archive_command`/`analyze_command`/`acceptance_command`/`resolve_command` は、設定のマージ結果に必ず存在しなければならない (MUST)。未設定のまま実行に移ろうとした場合、設定エラーとして失敗しなければならない (MUST)。

#### Scenario: worktree_command を設定できる

- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "worktree_command": "opencode run --cwd {workspace_dir} '/openspec:proposal'"
  }
  ```
- **WHEN** ユーザーがTUIの `+` キーで提案作成フローを開始する
- **THEN** `worktree_command` が使用される

#### Scenario: 必須コマンドが欠落している場合は設定ロード時にエラーになる

- **GIVEN** 設定のマージ結果に `archive_command` が存在しない
- **WHEN** 設定をロードする
- **THEN** 設定ロード時にエラーとして失敗する
- **AND** エラーメッセージに欠落しているコマンド名が含まれる

### Requirement: 設定ファイルの優先順位

オーケストレーターは以下の優先順位で設定ファイルを読み込み、項目ごとにマージしなければならない (MUST):

1. **カスタム設定**: `--config` で指定されたファイル
2. **プロジェクト設定**: `.cflx.jsonc` (プロジェクトルート)
3. **XDG グローバル設定 (環境変数)**: `$XDG_CONFIG_HOME/cflx/config.jsonc`
4. **XDG グローバル設定 (デフォルト)**: `~/.config/cflx/config.jsonc`
5. **プラットフォーム標準のグローバル設定**: `dirs::config_dir()/cflx/config.jsonc`

同一キーが複数の設定に存在する場合は、より優先度が高い設定の値で上書きしなければならない (MUST)。
優先度の高い設定に存在しないキーは、下位の設定の値を引き継がなければならない (MUST)。

設定のマージ完了後、必須コマンド (`apply_command`/`archive_command`/`analyze_command`/`acceptance_command`/`resolve_command`) が欠落している場合は、設定ロード時にエラーとして失敗しなければならない (MUST)。

`worktree_command` および `propose_command` は任意であり、未設定でもエラーにならない (MAY)。

#### Scenario: 設定ロード時に必須コマンドの欠落を検出する
- **GIVEN** 設定のマージ結果に `apply_command` が存在しない
- **WHEN** 設定をロードする
- **THEN** 設定ロード時にエラーとして失敗する
- **AND** エラーメッセージに `apply_command` が欠落している旨が含まれる

#### Scenario: プロジェクト設定が部分的でもグローバル設定を引き継ぐ
- **GIVEN** `~/.config/cflx/config.jsonc` に:
  ```jsonc
  { "archive_command": "global-archive {change_id}" }
  ```
- **AND** `.cflx.jsonc` に:
  ```jsonc
  { "hooks": { "on_start": "echo start" } }
  ```
- **WHEN** 設定を読み込む
- **THEN** `archive_command` は `global-archive {change_id}` のまま保持される
- **AND** `hooks.on_start` は `echo start` に設定される

#### Scenario: 同一キーは上位設定が優先される
- **GIVEN** `~/.config/cflx/config.jsonc` に `apply_command` が存在する
- **AND** `.cflx.jsonc` に別の `apply_command` が存在する
- **WHEN** 設定を読み込む
- **THEN** `.cflx.jsonc` の `apply_command` が使用される

#### Scenario: カスタム設定は最優先で上書きされる
- **GIVEN** `--config /tmp/custom.jsonc` が指定されている
- **AND** `/tmp/custom.jsonc` に `resolve_command` が存在する
- **AND** `.cflx.jsonc` に別の `resolve_command` が存在する
- **WHEN** 設定を読み込む
- **THEN** `/tmp/custom.jsonc` の `resolve_command` が使用される

#### Scenario: hooks のディープマージ
- **GIVEN** `~/.config/cflx/config.jsonc` に:
  ```jsonc
  { "hooks": { "on_start": "echo global" } }
  ```
- **AND** `.cflx.jsonc` に:
  ```jsonc
  { "hooks": { "pre_apply": "echo project" } }
  ```
- **WHEN** 設定を読み込む
- **THEN** `hooks.on_start` は `echo global` のまま保持される
- **AND** `hooks.pre_apply` は `echo project` に設定される

### Requirement: 依存関係分析コマンドの設定

`analyze_command` は LLM による依存関係分析に使用するコマンドを設定できなければならない (MUST)。このコマンドは `{prompt}` プレースホルダーを使用して分析プロンプトを受け取る。

#### Scenario: カスタム分析コマンドの使用

- **WHEN** `analyze_command` が `"claude-code '{prompt}'"` と設定されている
- **AND** 依存関係分析が実行される
- **THEN** `claude-code` にプロンプトが渡され、その出力が解析される

#### Scenario: analyze_command が未設定の場合は設定エラーになる

- **GIVEN** 設定のマージ結果に `analyze_command` が存在しない
- **WHEN** 依存関係分析が実行される
- **THEN** 設定エラーとして失敗する
- **AND** エラーメッセージに `analyze_command` が欠落している旨が含まれる
