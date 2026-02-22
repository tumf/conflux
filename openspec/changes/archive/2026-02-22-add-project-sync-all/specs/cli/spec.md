## ADDED Requirements

### Requirement: project sync --all による全件同期
CLI は `cflx project sync --all` をサポートし、登録済みプロジェクトをすべて `git/sync` で同期しなければならない（SHALL）。
同期は各プロジェクトの結果を個別に出力し、1 件でも失敗があれば非 0 で終了しなければならない（MUST）。

#### Scenario: 全プロジェクトを同期する
- **GIVEN** サーバに 2 件以上のプロジェクトが登録されている
- **WHEN** ユーザーが `cflx project sync --all` を実行する
- **THEN** CLI は `GET /api/v1/projects` で一覧を取得する
- **AND** 各プロジェクトに対して `POST /api/v1/projects/{id}/git/sync` を順に呼び出す
- **AND** 各プロジェクトの同期結果が表示される

#### Scenario: 失敗が含まれる場合は非 0 で終了する
- **GIVEN** 同期対象のうち 1 件が失敗する
- **WHEN** ユーザーが `cflx project sync --all` を実行する
- **THEN** CLI は他のプロジェクトの同期を継続する
- **AND** 最終的な終了コードは非 0 になる

#### Scenario: 対象が 0 件の場合はスキップする
- **GIVEN** サーバに登録済みプロジェクトが存在しない
- **WHEN** ユーザーが `cflx project sync --all` を実行する
- **THEN** CLI は同期対象が無い旨を表示する
- **AND** 同期リクエストは送信されない
