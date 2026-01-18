## MODIFIED Requirements
### Requirement: エージェントコマンドの設定ファイル

オーケストレーターは JSONC 形式の設定ファイルを通じてエージェントコマンドを設定できなければならない (MUST)。

設定可能なコマンドは以下の種類とする:

1. `apply_command` - 変更の適用コマンド
2. `archive_command` - 変更のアーカイブコマンド
3. `analyze_command` - 依存関係分析コマンド
4. `resolve_command` - Git マージの完了（merge/add/commit）や競合解消に使用するコマンド
5. `acceptance_command` - apply 後の受け入れ検査コマンド
6. `hooks` - 段階フックコマンド
7. `propose_command` - （後方互換のため残り得る）提案作成コマンド
8. `worktree_command` - TUIの `+` から起動される worktree 上の提案作成コマンド

#### Scenario: worktree_command を設定できる

- **GIVEN** `.cflx.jsonc` に以下の設定が存在する:
  ```jsonc
  {
    "worktree_command": "opencode run --cwd {workspace_dir} '/openspec:proposal'"
  }
  ```
- **WHEN** ユーザーがTUIの `+` キーで提案作成フローを開始する
- **THEN** `worktree_command` が使用される

## ADDED Requirements
### Requirement: Acceptance Command Configuration

オーケストレーターは `acceptance_command` と `acceptance_prompt` を設定項目として提供し、受け入れ検査を実行できなければならない（MUST）。

- `acceptance_command` は `{change_id}` と `{prompt}` プレースホルダーをサポートしなければならない（MUST）。
- acceptance 用のハードコードプロンプトを提供し、`acceptance_prompt` はその末尾に連結されなければならない（SHALL）。
- `acceptance_prompt` が未設定の場合は空文字として扱う（SHALL）。

#### Scenario: acceptance_command のプレースホルダー展開

- **GIVEN** `.cflx.jsonc` に以下が設定されている:
  ```jsonc
  {
    "acceptance_command": "agent acceptance {change_id} {prompt}",
    "acceptance_prompt": "Validate spec compliance."
  }
  ```
- **WHEN** change `add-feature` の acceptance を実行する
- **THEN** 実行されるコマンドは `agent acceptance add-feature <hardcoded acceptance prompt> Validate spec compliance.` となる

#### Scenario: acceptance_prompt 未設定

- **GIVEN** `acceptance_prompt` が設定されていない
- **WHEN** change `add-feature` の acceptance を実行する
- **THEN** `{prompt}` は `<hardcoded acceptance prompt>` のみを含む

### Requirement: Acceptance Prompt History Injection

オーケストレーターは `acceptance_command` 実行時に、ハードコード acceptance プロンプトと `acceptance_prompt` の後ろへ過去の acceptance 履歴コンテキストを追記しなければならない（SHALL）。

#### Scenario: 2回目の acceptance に履歴が含まれる

- **GIVEN** change `add-feature` に対する acceptance の1回目が失敗している
- **WHEN** 2回目の acceptance を実行する
- **THEN** `{prompt}` には `<hardcoded acceptance prompt>` が含まれる
- **AND** `{prompt}` には `acceptance_prompt` が含まれる
- **AND** `{prompt}` には `<last_acceptance attempt="1">` ブロックが含まれる

### Requirement: Templates include acceptance_command

`cflx init` のテンプレートは `acceptance_command` と `acceptance_prompt` を含めなければならない（SHALL）。

#### Scenario: OpenCode template includes acceptance_command

- **WHEN** `cflx init --template opencode` が実行される
- **THEN** 生成される設定に `acceptance_command` が含まれる
- **AND** `acceptance_prompt` のコメント設定が含まれる
