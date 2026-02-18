## 1. CLI と設定
- [x] 1.1 `cflx server` サブコマンドを追加する（確認: `src/cli.rs` にサブコマンド定義とヘルプ文言が追加されている）
- [x] 1.2 `server` 設定セクション（bind/port/auth/max_concurrent_total/data_dir）を追加する（確認: `src/config/mod.rs` と `src/config/defaults.rs` に構造体・既定値がある）
- [x] 1.3 非ループバック bind 時に `auth.mode=bearer_token` を必須化する検証を追加する（確認: 設定検証の unit test がある）

## 2. サーバ起動と永続化
- [x] 2.1 サーバエントリーポイントとルータ初期化を追加する（確認: `src/main.rs` から `server` 起動コードが呼ばれる）
- [x] 2.2 プロジェクトレジストリと永続化ストアを追加する（確認: `data_dir` に JSON を保存する実装と `tempfile` を使った unit test がある）
- [x] 2.3 `project_id` 生成（`md5(remote_url + "\n" + branch)` の先頭 16 文字）を実装する（確認: 入力が同じなら同一 ID になる unit test がある）

## 3. 排他制御と同時実行上限
- [x] 3.1 プロジェクト単位の排他ロックを追加する（確認: 同一プロジェクトに対する同時リクエストが直列化される unit test がある）
- [x] 3.2 `max_concurrent_total` のグローバルセマフォを導入する（確認: 同時実行数が上限を超えないことを検証する unit test がある）

## 4. API v1
- [x] 4.1 `/api/v1/projects` の一覧/追加/削除を実装する（確認: axum のテストで 200/201/204 を検証する）
- [x] 4.2 `/api/v1/projects/{id}/git/pull` と `/git/push` を実装する（確認: ローカル bare repo を使うテストで non-fast-forward を明示エラーにする）
- [x] 4.3 `/api/v1/projects/{id}/control/run|stop|retry` を実装する（確認: スタブ runner を使う unit test で呼び出しが記録される）
- [x] 4.4 bearer token 認証を実装する（確認: token あり/なしの API テストで 200/401 を検証する）

## 5. ポリシー確認
- [x] 5.1 サーバが `~/.wt/setup` を参照しないことを明記したガードを追加する（確認: `src/server/` に `~/.wt/setup` 参照がないことをコードレビューで確認できる）
