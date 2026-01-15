## MODIFIED Requirements
### Requirement: Parallel Execution Configuration
オーケストレーターは設定ファイルでparallel実行の設定を提供しなければならない（SHALL）。`parallel_mode` が未設定の場合、Gitリポジトリが検知されればparallelを既定で有効にし、検知されない場合は無効とする。`parallel_mode` が明示されている場合はその値を優先する。

#### Scenario: 未設定でGitが検知される場合はparallelを既定有効化
- **WHEN** config file does not contain `"parallel_mode"` key
- **AND** `.git` ディレクトリが存在する
- **THEN** parallel execution mode is enabled by default
- **AND** CLI `--parallel` flag is not required

#### Scenario: 未設定でGitが検知されない場合は無効
- **WHEN** config file does not contain `"parallel_mode"` key
- **AND** `.git` ディレクトリが存在しない
- **THEN** parallel execution mode is disabled

#### Scenario: parallel_mode false が指定されている場合は無効
- **WHEN** config file contains `"parallel_mode": false`
- **THEN** parallel execution mode is disabled
- **AND** `.git` ディレクトリが存在していても無効

#### Scenario: parallel_mode true が指定されている場合は有効
- **WHEN** config file contains `"parallel_mode": true`
- **THEN** parallel execution mode is enabled by default
- **AND** git repository is required (`.git` directory must exist)
