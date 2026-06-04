## Implementation Tasks

- [x] 1. 現在の control API の代表挙動を characterization test で固定する。
  verification: integration - `cargo test server::api::control::tests::test_toggle_change_selection_emits_change_update_without_waiting_full_state`、`cargo test server::api::control::tests::test_global_control_run_skips_rejected_changes`、`cargo test server::api::control::tests::test_stop_and_dequeue_change_deselects_and_returns_ok`。
  completion: selection、global run、stop/dequeue の status/body/state update がテストで確認できる。
- [x] 2. `src/server/api/control.rs` の handler/helper を責務別に分割し、route 登録と public surface を維持する。
  verification: unit - `cargo test server::api::control`。
  completion: 外部 route path、HTTP method、response type がリファクタ前と一致する。
- [x] 3. control API テスト fixture の重複 setup を共通 helper へ寄せ、テスト意図を route ごとに読みやすくする。
  verification: integration - `cargo test server::api::control`。
  completion: テストが fixture 構築より API 挙動の assertion を中心に読める状態になる。
- [x] 4. 全体の server API regressions を確認する。
  verification: integration - `cargo test server::api`。
  completion: control API 以外の server API テストにも後退がない。

## Future Work

- WebUI 側の control 操作 UX 改善は別 change で扱う。

## Final Validation

Expected archive gate: `cflx openspec validate refactor-server-control-api --archive-gate`
