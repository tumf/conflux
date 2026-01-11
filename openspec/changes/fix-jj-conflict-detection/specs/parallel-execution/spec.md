## ADDED Requirements

### Requirement: jj Merge Conflict Detection on Success

jj バックエンドで `jj new` によるマージコミット作成時、コマンドが成功ステータス（exit code 0）で終了した場合でもコンフリクトを検出し、適切なエラーを返さなければならない（MUST）。

jj は Git と異なり、コンフリクト状態のコミットを作成することが可能であり、`jj new` コマンドはコンフリクトがあっても成功ステータスで終了する。このため、コマンドの終了ステータスだけでなく、出力内容からコンフリクトを検出する必要がある。

#### Scenario: jj new が成功ステータスでコンフリクトを含む場合

- **WHEN** `jj new` コマンドが成功ステータス（exit code 0）で完了する
- **AND** stderr 出力に "conflict" または "Conflict" が含まれている
- **THEN** `VcsError::Conflict` エラーが返される
- **AND** `resolve_conflicts_with_retry` が呼び出される
- **AND** 設定された `resolve_command` が実行される

#### Scenario: jj new がコンフリクトなく成功する場合

- **WHEN** `jj new` コマンドが成功ステータス（exit code 0）で完了する
- **AND** stderr 出力にコンフリクト表示がない
- **THEN** マージコミットの revision ID が正常に返される
- **AND** `resolve_conflicts_with_retry` は呼び出されない
