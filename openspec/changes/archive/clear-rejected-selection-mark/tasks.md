## Implementation Tasks

- [x] Task 1: update reducer / orchestration 契約として `Rejected` terminal 遷移時に execution mark を clear する仕様とテスト観点を追加する（`openspec/specs/orchestration-state/spec.md` delta を追加し、対象 change のみ `selected=false` になる契約を明文化する） (verification: manual: `cflx openspec validate clear-rejected-selection-mark --strict` と `src/orchestration/state.rs`, `openspec/specs/orchestration-state/spec.md` の scenario review)
- [x] Task 2: update TUI state で rejected change が x マークを保持し続けないようにする（`src/tui/state.rs` の reducer display sync / rejected event handling を更新し、rejected row が `selected=false` になるよう揃える） (verification: unit: `cargo test` (src/tui/state.rs) で rejected transition clears selection / reactivation stays unselected)
- [x] Task 3: update server / Web snapshot で rejected change の `selected` が残らないようにする（`src/web/state.rs`, `src/server/api/control.rs` などの selection/snapshot 更新経路を見直し、rejected row を read-only terminal row として返す） (verification: integration: `cargo test` (src/web/state.rs, src/server/api/control.rs) で rejected change snapshot has `selected=false`)
- [x] Task 4: update dashboard / API 契約に rejected row の read-only selection semantics を追加する（`openspec/specs/server-api/spec.md`, `openspec/specs/web-monitoring/spec.md`, `openspec/specs/cli/spec.md` delta を追加し、checkbox/x mark の表示意味を terminal rejected に対して固定する） (verification: manual: `cflx openspec validate clear-rejected-selection-mark --strict` と `dashboard/src/components/ChangeRow.tsx`, `openspec/specs/server-api/spec.md`, `openspec/specs/web-monitoring/spec.md`, `openspec/specs/cli/spec.md` review)
- [x] Task 5: update 対象 change だけ mark clear し、他 change を巻き込まない回帰テストを追加する（TUI, reducer, server の少なくとも一系統ずつで単独 clear の回帰を固定する） (verification: unit+integration: `cargo test` で TUI/reducer/server regression coverage を確認)

## Future Work

- rejected reason を dashboard row に表示する追加 UX の検討
- 他 terminal state（archived / merged / stopped / error）の mark semantics を横断整理する設計見直し
