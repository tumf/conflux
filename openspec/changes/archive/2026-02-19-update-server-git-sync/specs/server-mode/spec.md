## MODIFIED Requirements
### Requirement: Git 同期の非 fast-forward を明示エラーにする
サーバは `git/sync` で non-fast-forward が発生した場合、明示的なエラー理由を返さなければならない（MUST）。

#### Scenario: non-fast-forward は理由付きで失敗する
- **GIVEN** リモートがローカルより進んでおり fast-forward できない
- **WHEN** `POST /api/v1/projects/{id}/git/sync` を呼び出す
- **THEN** サーバは失敗を返す
- **AND** 応答に `non_fast_forward` の理由が含まれる

## ADDED Requirements
### Requirement: Git 同期の統合 API
サーバは `git/pull` / `git/push` を統合する `POST /api/v1/projects/{id}/git/sync` を提供しなければならない（MUST）。

`sync` は pull と push の結果を 1 つのレスポンスで返さなければならない（MUST）。

#### Scenario: sync は pull/push の結果を返す
- **GIVEN** リモートとローカルに差分がある
- **WHEN** `POST /api/v1/projects/{id}/git/sync` を呼び出す
- **THEN** 応答には pull の結果と push の結果が含まれる

### Requirement: Git 同期の resolve 必須化
`git/sync` は resolve_command を必ず実行し、失敗時は同期失敗として返さなければならない（MUST）。

#### Scenario: resolve_command が実行される
- **GIVEN** `resolve_command` が設定されている
- **WHEN** `POST /api/v1/projects/{id}/git/sync` を呼び出す
- **THEN** サーバは resolve_command を実行する
- **AND** 応答に `resolve_command_ran=true` が含まれる

#### Scenario: resolve_command 未設定は失敗になる
- **GIVEN** `resolve_command` が未設定である
- **WHEN** `POST /api/v1/projects/{id}/git/sync` を呼び出す
- **THEN** サーバは失敗を返す
- **AND** 応答に設定不足の理由が含まれる
