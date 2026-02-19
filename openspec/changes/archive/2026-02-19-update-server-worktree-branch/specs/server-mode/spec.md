## MODIFIED Requirements
### Requirement: プロジェクト追加時の自動クローン
サーバは `POST /api/v1/projects` の成功時に、指定された `remote_url` と `branch` を検証し、サーバの `data_dir` 配下にローカル clone と作業ツリーを準備しなければならない（MUST）。

作業ツリーは base ブランチとは別の server 専用ブランチで作成しなければならない（MUST）。

#### Scenario: 追加時にローカルクローンが用意される
- **WHEN** クライアントが `POST /api/v1/projects` に `remote_url` と `branch` を送信する
- **THEN** サーバは `branch` の存在を検証する
- **AND** `data_dir/<project_id>` に bare clone を作成または更新する
- **AND** `data_dir/worktrees/<project_id>/<branch>` に作業ツリーを作成または更新する
- **AND** 作業ツリーは server 専用ブランチを checkout している
- **AND** すべて成功した場合に `201` を返す

#### Scenario: クローン失敗時は追加を完了しない
- **GIVEN** git clone または worktree 作成が失敗する
- **WHEN** クライアントが `POST /api/v1/projects` を呼び出す
- **THEN** サーバはエラーを返す
- **AND** 追加対象のプロジェクトは registry に残らない
