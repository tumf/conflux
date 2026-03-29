## MODIFIED Requirements

### Requirement: フック設定の詳細オプション

オーケストレータはフックごとに `continue_on_failure`、`timeout`、および `git_commit_no_verify` を設定できなければならない (MUST)。

- `continue_on_failure` のデフォルト値は `true` とする
- `timeout` のデフォルト値は 60 秒とする
- `git_commit_no_verify` のデフォルト値は `false` とする

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

#### Scenario: git_commit_no_verify を明示的に有効化する

- **GIVEN** `hooks.on_merged` が以下のように設定されている:
  ```jsonc
  {
    "hooks": {
      "on_merged": {
        "command": "make bump-patch",
        "timeout": 60,
        "git_commit_no_verify": true
      }
    }
  }
  ```
- **WHEN** オーケストレータが設定を読み込む
- **THEN** `on_merged` は `git_commit_no_verify=true` として扱われる

#### Scenario: git_commit_no_verify の省略時デフォルト

- **GIVEN** 詳細 hook 設定に `git_commit_no_verify` が含まれていない
- **WHEN** オーケストレータが設定を読み込む
- **THEN** `git_commit_no_verify` は `false` として扱われる
