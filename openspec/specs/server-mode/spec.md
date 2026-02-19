# server-mode Specification

## Purpose
TBD - created by archiving change add-server-daemon. Update Purpose after archive.
## Requirements
### Requirement: サーバ起動はグローバル設定のみを使用する
`cflx server` はカレントディレクトリの `.cflx.jsonc` を読み込まず、グローバル設定のみで起動しなければならない（MUST）。

#### Scenario: プロジェクト設定を読まずに起動する
- **GIVEN** カレントディレクトリに `.cflx.jsonc` が存在する
- **WHEN** ユーザーが `cflx server` を起動する
- **THEN** サーバは `.cflx.jsonc` を読み込まない
- **AND** `~/.config/cflx/config.jsonc` などのグローバル設定のみを使用する

### Requirement: 非ループバック bind は bearer token 必須
`server.bind` がループバック以外の場合、`auth.mode=bearer_token` を必須としなければならない（MUST）。

#### Scenario: 非ループバック bind の起動は失敗する
- **GIVEN** `server.bind=0.0.0.0`
- **AND** `server.auth.mode=none`
- **WHEN** `cflx server` を起動する
- **THEN** 起動はエラーで失敗する

### Requirement: プロジェクト識別子と永続化
サーバは `remote_url` と `branch` を入力として決定的な `project_id` を生成し、永続化しなければならない（MUST）。

#### Scenario: 同一入力は同一 ID になる
- **GIVEN** `remote_url` と `branch` が同一である
- **WHEN** 2 回 `project_id` を生成する
- **THEN** 生成結果は同一になる

#### Scenario: 追加したプロジェクトが一覧に含まれる
- **WHEN** クライアントが `POST /api/v1/projects` に `remote_url` と `branch` を送信する
- **THEN** サーバは `project_id` を生成して保存する
- **AND** `GET /api/v1/projects` に新しいプロジェクトが含まれる

### Requirement: リポジトリ操作の排他
サーバは同一 `project_id` に対する Git 操作および実行制御を直列化しなければならない（MUST）。

#### Scenario: 同一プロジェクトの同時操作は直列化される
- **GIVEN** 同一 `project_id` に対して 2 つの操作要求が同時に送られる
- **WHEN** サーバが要求を処理する
- **THEN** 2 つの操作は同時に実行されない

### Requirement: API v1 を提供する
サーバは `api/v1` 名前空間でプロジェクト管理と実行制御の API を提供しなければならない（SHALL）。

#### Scenario: プロジェクト管理 API が応答する
- **WHEN** クライアントが `GET /api/v1/projects` を呼び出す
- **THEN** サーバは 200 で一覧を返す

### Requirement: Git 同期の非 fast-forward を明示エラーにする
サーバは `git/pull` と `git/push` で non-fast-forward が発生した場合、`auto_resolve` が未指定または false のときは明示的なエラー理由を返さなければならない（MUST）。

#### Scenario: non-fast-forward は理由付きで失敗する
- **GIVEN** リモートがローカルより進んでおり fast-forward できない
- **AND** `auto_resolve` が未指定または false である
- **WHEN** `POST /api/v1/projects/{id}/git/pull` を呼び出す
- **THEN** サーバは失敗を返す
- **AND** 応答に `non_fast_forward` の理由が含まれる

### Requirement: グローバル同時実行上限
サーバは全プロジェクト合算の同時実行上限 `server.max_concurrent_total` を適用しなければならない（MUST）。

#### Scenario: 同時実行数が上限を超えない
- **GIVEN** `server.max_concurrent_total=4`
- **WHEN** 複数プロジェクトの実行要求が同時に発生する
- **THEN** 実行中のワーカー数は常に 4 以下になる

### Requirement: `~/.wt/setup` を参照しない
サーバモードは `~/.wt/setup` を読み込んだり実行したりしてはならない（MUST NOT）。

#### Scenario: `~/.wt/setup` が存在しても無視される
- **GIVEN** `~/.wt/setup` が存在する
- **WHEN** サーバが起動またはプロジェクト操作を行う
- **THEN** `~/.wt/setup` は参照されない

### Requirement: プロジェクト追加時の自動クローン
サーバは `POST /api/v1/projects` の成功時に、指定された `remote_url` と `branch` を検証し、サーバの `data_dir` 配下にローカル clone と作業ツリーを準備しなければならない（MUST）。

#### Scenario: 追加時にローカルクローンが用意される
- **WHEN** クライアントが `POST /api/v1/projects` に `remote_url` と `branch` を送信する
- **THEN** サーバは `branch` の存在を検証する
- **AND** `data_dir/<project_id>` に bare clone を作成または更新する
- **AND** `data_dir/worktrees/<project_id>/<branch>` に作業ツリーを作成または更新する
- **AND** すべて成功した場合に `201` を返す

#### Scenario: クローン失敗時は追加を完了しない
- **GIVEN** git clone または worktree 作成が失敗する
- **WHEN** クライアントが `POST /api/v1/projects` を呼び出す
- **THEN** サーバはエラーを返す
- **AND** 追加対象のプロジェクトは registry に残らない

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

### Requirement: サーバの auto_resolve は共通 resolve_command を使用する
サーバは auto_resolve における解決コマンドとして、設定マージ済みのトップレベル `resolve_command` を使用しなければならない（MUST）。

#### Scenario: auto_resolve で共通 resolve_command が使われる
- **GIVEN** 設定のマージ結果に `resolve_command` が存在する
- **AND** `auto_resolve=true` が指定されている
- **WHEN** サーバが `git/pull` を処理する
- **THEN** サーバはトップレベル `resolve_command` を実行する
