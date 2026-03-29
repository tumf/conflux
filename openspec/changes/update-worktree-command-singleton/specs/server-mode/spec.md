## MODIFIED Requirements

### Requirement: リポジトリ操作の排他
サーバは同一 `project_id` に対する Git 操作および実行制御を直列化しなければならない（MUST）。

server-mode の API で worktree root（base を含む）を変更またはその上でコマンドを実行する操作は、対象 root ごとに 1 件だけ同時実行可能でなければならない（MUST）。

busy な root に対する新規要求は待機キューに入れず、即時 `409 Conflict` を返さなければならない（MUST）。

#### Scenario: 同一 base root の同時 sync は拒否される
- **GIVEN** `POST /api/v1/projects/{id}/git/sync` により base root 上の sync が実行中である
- **WHEN** 同じ `project_id` の base root を対象とする別の sync 要求が届く
- **THEN** サーバは要求を待機させない
- **AND** `409 Conflict` を返す
- **AND** 応答には対象 root が busy である理由が含まれる

#### Scenario: 同一 worktree root の競合操作は拒否される
- **GIVEN** ある project の特定 worktree root 上で apply または merge が実行中である
- **WHEN** 同じ worktree root を対象とする別の API 操作が届く
- **THEN** サーバはその要求を開始しない
- **AND** `409 Conflict` を返す

### Requirement: リモートTUI向けのログ配信
サーバは WebSocket の状態更新に、プロジェクト実行中のログを含めて配信しなければならない（MUST）。

ログは少なくとも以下を含む:
- `project_id`
- `change_id`（不明な場合は `null`）
- `operation`
- `iteration`
- `message`
- `level`
- `timestamp`

`git/sync` が内部で実行する resolve_command の stdout/stderr と開始・完了・失敗イベントも、対象 `project_id` のログとして同じ経路で配信しなければならない（MUST）。

#### Scenario: sync の resolve 出力が project log に流れる
- **GIVEN** `POST /api/v1/projects/{id}/git/sync` が resolve_command を実行する
- **WHEN** resolve_command が stdout または stderr を出力する
- **THEN** サーバはその行を `project_id={id}` のログイベントとして WebSocket に配信する
- **AND** dashboard の project log で表示できる

## ADDED Requirements

### Requirement: server-mode は active command 状態を公開する
server-mode は WebUI が worktree root ごとの busy 状態を復元できるよう、実行中 command の一覧を状態 API と WebSocket `full_state` に含めなければならない（MUST）。

各 active command は少なくとも `project_id`、root の識別情報、`operation`、開始時刻を含まなければならない（MUST）。

#### Scenario: リロード後も sync 中表示が復元される
- **GIVEN** ある project の base root で sync が実行中である
- **WHEN** ユーザーが dashboard をリロードし、最新の `full_state` を受信する
- **THEN** 受信 payload にその root の active command 情報が含まれる
- **AND** クライアントは Sync ボタンを disabled のまま表示できる
