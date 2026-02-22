## 1. CLI オプションと実行分岐

- [x] 1.1 `cflx project sync` に `--all` フラグを追加する（確認: `src/cli.rs` で `--all` がパース可能）
- [x] 1.2 `--all` 指定時は project 一覧取得 → 各 project の同期を実行する分岐を追加する（確認: `src/main.rs` で `--all` が別経路になる）
- [x] 1.3 `--all` と `project_id` が同時指定された場合はエラーにする（確認: コマンド実行前に検証される）

## 2. 進捗と終了コード

- [x] 2.1 各プロジェクトの同期結果を個別に出力する（確認: `sync --all` 実行時に project_id ごとの出力がある）
- [x] 2.2 失敗が 1 件でもあれば非 0 で終了する（確認: 失敗を含むテストで exit code が非 0）
- [x] 2.3 対象 0 件のときは同期を行わずメッセージを表示する（確認: `GET /api/v1/projects` が空配列のときの出力）

## 3. テスト

- [x] 3.1 `sync --all` の clap パーステストを追加する（確認: `cflx project sync --all` がパースできる）
- [x] 3.2 モック HTTP サーバで一覧取得と同期呼び出しの順序を検証する（確認: `GET /api/v1/projects` 後に `POST /api/v1/projects/{id}/git/sync` が呼ばれる）
- [x] 3.3 同期失敗が含まれる場合の終了コードを検証する（確認: 1 件でも失敗時に非 0）

## Acceptance #1 Failure Follow-up

- [x] Ensure `git status --porcelain` is empty by resolving uncommitted changes in `src/cli.rs`, `src/remote/client.rs`, and `src/remote/test_helpers.rs` before the next acceptance run.
- [x] Fix flakiness in `remote::client::tests::test_list_then_sync_ordering` so `cargo test` passes reliably in the full suite (update `src/remote/test_helpers.rs` `spawn_mock_http_server_ordered` or equivalent test/client setup to handle sequential sync requests deterministically).

## Acceptance #2 Failure Follow-up

- [x] Fix `remote::client::tests::test_list_then_sync_ordering` instability so full-suite `cargo test -q` passes consistently; latest run still fails at `src/remote/client.rs:364` with `Failed to sync project 'proj-2': error sending request` while using `spawn_mock_http_server_ordered` in `src/remote/test_helpers.rs:264`.
