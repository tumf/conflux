## MODIFIED Requirements

### Requirement: `~/.wt/setup` を参照しない
サーバモードは `~/.wt/setup` を読み込んだり実行したりしてはならない（MUST NOT）。

ただし、server mode が作成する各プロジェクトの worktree については、対象リポジトリ直下の `.wt/setup` が存在する場合に実行しなければならない（MUST）。

#### Scenario: `~/.wt/setup` が存在しても無視される
- **GIVEN** `~/.wt/setup` が存在する
- **WHEN** サーバが起動またはプロジェクト操作を行う
- **THEN** `~/.wt/setup` は参照されない

#### Scenario: server project 追加時に repo-root の `.wt/setup` が実行される
- **GIVEN** 追加対象リポジトリのルートに `.wt/setup` が存在する
- **WHEN** クライアントが `POST /api/v1/projects` を呼び出す
- **THEN** サーバは作成した worktree 上で `.wt/setup` を実行する
- **AND** `ROOT_WORKTREE_PATH` は対象リポジトリルートを指す

#### Scenario: repo-root `.wt/setup` が失敗した場合は追加を完了しない
- **GIVEN** 追加対象リポジトリのルートに `.wt/setup` が存在する
- **AND** `.wt/setup` が非0終了する
- **WHEN** クライアントが `POST /api/v1/projects` を呼び出す
- **THEN** サーバはエラーを返す
- **AND** 追加対象のプロジェクトは registry に残らない
