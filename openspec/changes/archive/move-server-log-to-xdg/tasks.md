## Implementation Tasks

- [x] Task 1: `get_server_log_path()` を `src/config/defaults.rs` に追加 (verification: `cargo test get_server_log_path` が通る)
- [x] Task 2: `generate_plist()` のシグネチャに `log_path: &Path` を追加し、plist テンプレート内の `/tmp/cflx-server.log` を動的パスに置換 (verification: `src/service/mod.rs` に `/tmp` のハードコードが残らない)
- [x] Task 3: `install()` 内でログディレクトリを `create_dir_all` で事前作成 (verification: ディレクトリ未存在時にも install が成功する)
- [x] Task 4: `get_server_log_path()` の単体テストを追加（XDG_STATE_HOME 設定時、未設定時、ホームなし時の3パターン） (verification: `cargo test get_server_log_path`)
- [x] Task 5: `cargo fmt --check && cargo clippy -- -D warnings` が通ることを確認 (verification: CI green)

## Future Work

- 既存 `/tmp/cflx-server.log` を使用中のユーザへのマイグレーションガイド
