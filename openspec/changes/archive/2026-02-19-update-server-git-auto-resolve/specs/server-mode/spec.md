## MODIFIED Requirements
### Requirement: Git 同期の非 fast-forward を明示エラーにする
サーバは `git/pull` と `git/push` で non-fast-forward が発生した場合、`auto_resolve` が未指定または false のときは明示的なエラー理由を返さなければならない（MUST）。

#### Scenario: non-fast-forward は理由付きで失敗する
- **GIVEN** リモートがローカルより進んでおり fast-forward できない
- **AND** `auto_resolve` が未指定または false である
- **WHEN** `POST /api/v1/projects/{id}/git/pull` を呼び出す
- **THEN** サーバは失敗を返す
- **AND** 応答に `non_fast_forward` の理由が含まれる

## ADDED Requirements
### Requirement: Git 同期の auto_resolve オプション
サーバは `git/pull` と `git/push` で `auto_resolve=true` が指定された場合、non-fast-forward を検知したら resolve_command を実行し、成功時のみ処理を継続しなければならない（MUST）。

#### Scenario: auto_resolve で resolve_command が実行される
- **GIVEN** non-fast-forward が発生している
- **AND** `auto_resolve=true` が指定されている
- **WHEN** `POST /api/v1/projects/{id}/git/pull` を呼び出す
- **THEN** サーバは resolve_command を実行する
- **AND** 応答に `resolve_command_ran=true` が含まれる

#### Scenario: resolve_command が失敗した場合は失敗を返す
- **GIVEN** non-fast-forward が発生している
- **AND** `auto_resolve=true` が指定されている
- **AND** resolve_command が失敗する
- **WHEN** `POST /api/v1/projects/{id}/git/push` を呼び出す
- **THEN** サーバは失敗を返す
- **AND** 応答に `resolve_command_ran=true` が含まれる
