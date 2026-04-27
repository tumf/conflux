## 1. Implementation

- [x] 1.1 `openspec/specs/orchestration-state/spec.md` と `openspec/specs/tui-state/spec.md` の delta を追加し、execution-oriented active listing と TUI read-only rejected row の責務分離を明文化する
- [x] 1.2 Implement runtime queue handling in TUI change refresh so rejected marker-bearing changes are materialized as read-only rows (`src/tui/state.rs`, `src/tui/runner.rs`, 必要なら表示用 helper) (verification: integration command `cargo test tui::state::tests::`)
- [x] 1.3 rejected row の表示 semantics を固定する（status=`rejected`, reducer vocabulary と整合する色、`selected=false` 維持）
- [x] 1.4 rejected row の操作ガードを追加する（Space / `@` / `F5` / resume 系操作で x マークや queue intent を付けられないようにする）
- [x] 1.5 marker removal による再活性化回帰を固定する（`REJECTED.md` 削除後は `not queued` かつ unselected で戻ることを確認する）

## 2. Tests

- [x] 2.1 strict validation と spec scenario review を実行する（verification: `cflx openspec validate show-rejected-changes-in-tui --strict`）
- [x] 2.2 integration テストで rejected change が refresh 後に read-only row として表示されることを確認する（verification: `cargo test tui::state::tests::`）
- [x] 2.3 unit テストで rejected row の status/color/selection semantics を確認する（verification: `cargo test tui::state::tests::`）
- [x] 2.4 integration テストで key handling / reducer sync 時に rejected row が unselected を維持することを確認する（verification: `cargo test tui::state::tests::`）
- [x] 2.5 integration テストで `REJECTED.md` 削除後の再活性化時に `not queued` かつ unselected で戻ることを確認する（verification: `cargo test tui::state::tests::`）

## Future Work

- rejected reason を TUI detail popup や tooltip で見せる UX 改善
- terminal rows 全般の filter / grouping 表示の見直し
