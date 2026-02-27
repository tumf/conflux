## Implementation Tasks

- [x] 1.1 Logsビューの折り返し仕様をdelta specに反映する（verification: `openspec/changes/update-tui-log-wrap-no-indent/specs/tui-architecture/spec.md` が追加され、strict validateが通る）
- [x] 1.2 Logsビューの折り返し実装を更新し、継続行をインデントしない表示にする（verification: TUIで長文ログがexp通りに表示される）
- [x] 1.3 折り返し用ヘルパのテストを更新/追加し、継続行がインデントされないことを検証する（verification: `cargo test tui::render::tests::test_logs_wrap_*` が成功する）
- [x] 1.4 長文ログが折り返されても最新ログが表示範囲から外れないことのテストを維持する（verification: 既存の表示範囲テストが成功する）
- [x] 1.5 `cargo fmt` / `cargo clippy -- -D warnings` / `cargo test` を実行し、すべて成功させる（verification: コマンドがすべて成功する）

## Future Work

- なし
