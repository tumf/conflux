## Implementation Tasks

- [ ] Task 1: reducer / orchestration 契約として `Rejected` terminal 遷移時に execution mark を clear する仕様とテスト観点を追加する（`openspec/specs/orchestration-state/spec.md` delta を追加し、対象 change のみ `selected=false` になる契約を明文化する） (verification: strict validate + spec scenario review)
- [ ] Task 2: TUI state で rejected change が x マークを保持し続けないようにする（`src/tui/state.rs` の reducer display sync / rejected event handling を更新し、rejected row が `selected=false` になるよう揃える） (verification: `cargo test` で TUI state tests に rejected transition clears selection / reactivation stays unselected を追加)
- [ ] Task 3: server / Web snapshot で rejected change の `selected` が残らないようにする（`src/web/state.rs`, `src/server/api/control.rs` などの selection/snapshot 更新経路を見直し、rejected row を read-only terminal row として返す） (verification: server/web tests で rejected change snapshot has `selected=false` を追加)
- [ ] Task 4: dashboard / API 契約に rejected row の read-only selection semantics を追加する（`openspec/specs/server-api/spec.md`, `openspec/specs/web-monitoring/spec.md`, `openspec/specs/cli/spec.md` delta を追加し、checkbox/x mark の表示意味を terminal rejected に対して固定する） (verification: strict validate + affected UI contract review)
- [ ] Task 5: 対象 change だけ mark clear し、他 change を巻き込まない回帰テストを追加する（TUI, reducer, server の少なくとも一系統ずつで単独 clear の回帰を固定する） (verification: targeted `cargo test` names for TUI/reducer/server coverage)

## Future Work

- rejected reason を dashboard row に表示する追加 UX の検討
- 他 terminal state（archived / merged / stopped / error）の mark semantics を横断整理する設計見直し
