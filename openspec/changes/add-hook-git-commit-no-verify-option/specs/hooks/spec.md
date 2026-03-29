## MODIFIED Requirements

### Requirement: Hook configuration format

Hook configuration SHALL support both simple string form and detailed object form.

Detailed hook configuration SHALL support `command`, `timeout`, `continue_on_failure`, and `git_commit_no_verify` fields.

`git_commit_no_verify` SHALL default to `false` when omitted.

#### Scenario: Simple string hook

- **GIVEN** config contains:
  ```jsonc
  {
    "hooks": {
      "on_change_start": "jj new -m '{change_id}'"
    }
  }
  ```
- **WHEN** orchestrator loads the config
- **THEN** the hook is registered with default timeout (60s) and continue_on_failure (true)
- **AND** `git_commit_no_verify` is treated as false

#### Scenario: Detailed hook configuration

- **GIVEN** config contains:
  ```jsonc
  {
    "hooks": {
      "on_change_start": {
        "command": "jj new -m '{change_id}'",
        "timeout": 30,
        "continue_on_failure": false
      }
    }
  }
  ```
- **WHEN** orchestrator loads the config
- **THEN** the hook uses timeout=30s and continue_on_failure=false
- **AND** `git_commit_no_verify` is treated as false

#### Scenario: Detailed hook configuration enables git commit no-verify

- **GIVEN** config contains:
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
- **WHEN** orchestrator loads the config
- **THEN** the hook is registered with `git_commit_no_verify=true`

### Requirement: フックのコンテキスト（プレースホルダーと環境変数）

オーケストレータはフック実行時に、コマンド文字列内のプレースホルダーを展開し、同等の情報を環境変数としても提供しなければならない (MUST)。

Detailed hook options that affect downstream command behavior SHALL also be exposed to the hook execution environment in machine-readable form.

#### Scenario: git_commit_no_verify is exposed to hook command

- **GIVEN** `hooks.on_merged.git_commit_no_verify` is `true`
- **WHEN** `on_merged` is executed
- **THEN** the hook child process receives an environment variable indicating git commit verification should be skipped
- **AND** the variable value is machine-readable and unambiguous

#### Scenario: git_commit_no_verify defaults to false in environment

- **GIVEN** a detailed hook configuration omits `git_commit_no_verify`
- **WHEN** the hook is executed
- **THEN** the hook child process does not receive a contradictory true value for git commit verification bypass
