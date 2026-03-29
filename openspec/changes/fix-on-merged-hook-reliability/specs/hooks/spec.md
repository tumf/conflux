## MODIFIED Requirements

### Requirement: Hook configuration format

Hook configuration SHALL support both simple string form and detailed object form.

The detailed object form SHALL support the following fields:
- `command` (string, required): The command to execute
- `timeout` (u64, default 60): Timeout in seconds
- `continue_on_failure` (bool, default true): Whether to continue if the hook fails
- `git_commit_no_verify` (bool, default false): Whether downstream git commits should skip verification hooks
- `max_retries` (u32, default 0): Number of retries on non-zero exit before applying `continue_on_failure` logic
- `retry_delay_secs` (u64, default 3): Delay in seconds between retries

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
- **THEN** the hook is registered with default timeout (60s), continue_on_failure (true), max_retries (0), and retry_delay_secs (3)

#### Scenario: Detailed hook configuration with retry

- **GIVEN** config contains:
  ```jsonc
  {
    "hooks": {
      "on_merged": {
        "command": "make bump-patch",
        "timeout": 120,
        "git_commit_no_verify": true,
        "max_retries": 3,
        "retry_delay_secs": 3
      }
    }
  }
  ```
- **WHEN** orchestrator loads the config
- **THEN** the hook uses timeout=120s, git_commit_no_verify=true, max_retries=3, and retry_delay_secs=3

#### Scenario: Hook retry on failure

- **GIVEN** a hook is configured with `max_retries: 2` and `retry_delay_secs: 3`
- **AND** the hook command exits with non-zero status on the first attempt
- **WHEN** the hook is executed
- **THEN** the orchestrator waits 3 seconds and retries
- **AND** if the retry succeeds, the hook is considered successful
- **AND** if all retries fail, `continue_on_failure` logic is applied

#### Scenario: Default max_retries is zero (no retry)

- **GIVEN** a hook is configured without `max_retries`
- **AND** the hook command exits with non-zero status
- **WHEN** the hook is executed
- **THEN** `continue_on_failure` logic is applied immediately without retry

### Requirement: on_merged hook

オーケストレーターはchangeがbase branchにマージされた直後に`on_merged`フックを実行しなければならない（SHALL）。

`on_merged`はマージ成功時のみ1回実行され、マージ失敗時には実行しない。

parallelモードでは、自動マージが成功した全ての経路で`on_merged`を実行しなければならない（SHALL）。

`on_merged` フック実行前に、`.git/index.lock` ファイルの解放を待機しなければならない（SHALL）。最大待機時間は `hooks.index_lock_wait_secs`（デフォルト 10 秒）で設定可能。

#### Scenario: Parallelモードで自動マージ完了
- **GIVEN** `hooks.on_merged`が`echo 'Merged {change_id}'`に設定されている
- **WHEN** parallelモードでchange`change-a`がbase branchにマージされ`MergeCompleted`が発行される
- **THEN** `on_merged`が`{change_id}=change-a`で実行される

#### Scenario: Parallelモードでarchive直後に即時マージ成功
- **GIVEN** `hooks.on_merged`が`echo 'Merged {change_id}'`に設定されている
- **AND** parallelモードでchange`change-a`がarchive完了後に即時マージされる
- **WHEN** マージが成功する
- **THEN** `on_merged`が`{change_id}=change-a`で実行される

#### Scenario: TUI Worktreeの手動マージ完了
- **GIVEN** `hooks.on_merged`が設定されている
- **AND** worktreeブランチ`change-a`をMキーでマージする
- **WHEN** `BranchMergeCompleted`が発行される
- **THEN** `on_merged`が`{change_id}=change-a`で実行される

#### Scenario: serial(run)でのマージ相当
- **GIVEN** runモード（非parallel）でchange`change-a`を処理している
- **WHEN** archiveが成功し、base branchに変更が反映済みと確認できる
- **THEN** `on_merged`が`{change_id}=change-a`で実行される

#### Scenario: index.lock 待機後にフック実行

- **GIVEN** `hooks.on_merged` が設定されている
- **AND** `.git/index.lock` ファイルが存在する
- **WHEN** `on_merged` フックが実行される
- **THEN** オーケストレーターは `.git/index.lock` の解放を最大 `index_lock_wait_secs` 秒（デフォルト 10）待機する
- **AND** 解放後にフックコマンドを実行する

#### Scenario: index.lock 待機タイムアウト

- **GIVEN** `hooks.on_merged` が設定されている
- **AND** `.git/index.lock` ファイルが `index_lock_wait_secs` 秒を超えて存在し続ける
- **WHEN** `on_merged` フックが実行される
- **THEN** オーケストレーターは警告ログを出力してフックコマンドの実行を試行する

### Requirement: Hook execution working directory

フックコマンドは常にリポジトリルートディレクトリで実行されなければならない（SHALL）。

`HookRunner` は `repo_root` パスを保持し、`execute_hook()` でコマンドの作業ディレクトリとして設定しなければならない（SHALL）。

#### Scenario: フックがリポジトリルートで実行される

- **GIVEN** リポジトリルートが `/path/to/repo` である
- **AND** フックコマンドが `pwd` に設定されている
- **WHEN** フックが実行される
- **THEN** コマンドの出力は `/path/to/repo` である

#### Scenario: parallel mode worktree からのフック実行

- **GIVEN** parallel mode で worktree `/tmp/worktrees/change-a` が使用されている
- **AND** リポジトリルートが `/path/to/repo` である
- **WHEN** `on_merged` フックが実行される
- **THEN** コマンドはリポジトリルート `/path/to/repo` で実行される
