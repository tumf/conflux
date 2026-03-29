## MODIFIED Requirements

### Requirement: Git 同期の resolve 必須化

`git/sync` は resolve_command を必ず実行し、失敗時は同期失敗として返さなければならない（MUST）。server mode は `resolve_command` の `{prompt}` 展開において、他の command template と同じ shell-escaping / quoted-template 互換ルールを適用しなければならない（MUST）。

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

#### Scenario: quoted resolve_command template works during git sync
- **GIVEN** `resolve_command` が `"opencode run --agent code --model kani/kani/deep '{prompt}'"` の形式で設定されている
- **AND** `git/sync` が multi-line prompt を生成する
- **WHEN** サーバが `resolve_command` を実行する
- **THEN** prompt は 1 つの引数として渡される
- **AND** クォート崩れに起因する exit code 127 を返さない
