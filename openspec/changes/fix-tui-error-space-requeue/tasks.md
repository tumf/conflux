## Implementation Tasks

- [ ] Task 1: Error 遷移時に execution mark を clear する共通ロジックを実装する（`src/tui/state.rs` の Error 遷移ハンドラ群と、サーバー側 state/snapshot 更新経路を確認し、Error change が `selected = false` になるよう揃える） (verification: TUI state テストと server registry/API テストで Error change が未選択として観測される)
- [ ] Task 2: TUI で Error change を再マークしたとき、次回再開/実行で再キュー対象として扱うようにする（`src/tui/state.rs` の Running/Stopped モード toggle と resume フローを更新） (verification: `cargo test` + 新規テストで Error → mark set → resume により queued へ戻る)
- [ ] Task 3: API / Dashboard で Error change を toggle したとき、その `selected` 状態が WebSocket / state snapshot に反映され、次回 Run の対象に含まれるようにする（`src/server/registry.rs`, `src/server/api.rs`） (verification: API テストで Error change の toggle 後に `selected: true` が返り、Run 対象選定テストで含まれる)
- [ ] Task 4: TUI / API の双方で Error change の mark clear / re-mark semantics を回帰テストで固定する（TUI state tests, server API/registry tests） (verification: targeted `cargo test` names covering TUI and server selection behavior)
- [ ] Task 5: Error change の UI ヒント/表示を更新し、「再マークで再実行対象になる」ことが TUI 上で分かるようにする（`src/tui/render.rs`） (verification: render テストまたは snapshot テスト)
