## Implementation Tasks

- [x] Task 1: Error 遷移時に execution mark を clear する共通ロジックを実装する（`src/tui/state.rs` の Error 遷移ハンドラ群と、サーバー側 state/snapshot 更新経路を確認し、Error change が `selected = false` になるよう揃える） (verification: TUI state テストと server registry/API テストで Error change が未選択として観測される)
- [x] Task 2: TUI で Error change を再マークしたとき、次回再開/実行で再キュー対象として扱うようにする（`src/tui/state.rs` の Running/Stopped モード toggle と resume フローを更新） (verification: `cargo test` + 新規テストで Error → mark set → resume により queued へ戻る)
- [x] Task 3: API / Dashboard で Error change を toggle したとき、その `selected` 状態が WebSocket / state snapshot に反映され、次回 Run の対象に含まれるようにする（`src/server/registry.rs`, `src/server/api.rs`） (verification: API テストで Error change の toggle 後に `selected: true` が返り、Run 対象選定テストで含まれる)
- [x] Task 4: TUI / API の双方で Error change の mark clear / re-mark semantics を回帰テストで固定する（TUI state tests, server API/registry tests） (verification: targeted `cargo test` names covering TUI and server selection behavior)
- [x] Task 5: Error change の UI ヒント/表示を更新し、「再マークで再実行対象になる」ことが TUI 上で分かるようにする（`src/tui/render.rs`） (verification: render テストまたは snapshot テスト)

## Acceptance #1 Failure Follow-up

- [x] サーバー側で Error change を state snapshot / Run 対象判定に反映し、未再マークの Error change が除外され再マーク後のみ再実行対象になるよう実装する
- [x] 上記の server API / registry 挙動を Error 状態つきのテストで固定し、誤って非 Error change の toggle テストだけで完了扱いしないようにする
- [x] 作業ツリーをクリーンにしてから acceptance を再実行する

## Acceptance #2 Failure Follow-up

- [x] `toggle_all_changes()` の既定選択値を Error change では `false` に揃え、server selection semantics を一貫させる
- [x] bulk toggle 経路の Error change 挙動を固定する server API / registry テストを追加する

## Acceptance #3 Failure Follow-up

- [x] `toggle_all_change_selection()` で bulk toggle 時に `clear_change_error()` しないよう修正し、Error change の再マーク semantics を個別 toggle と揃える
- [x] bulk toggle の server API テストを spec 通りに更新し、再マーク後も `status: "error"` が維持されつつ次回 Run 対象になることを固定する
- [x] `tasks.md` の完了チェックを実装実態に合わせて真実に修正し、作業ツリーをクリーンにしてから acceptance を再実行する
