## Implementation Tasks

- [x] Task 1: `openspec/specs/orchestration-state/spec.md` と `openspec/specs/tui-state/spec.md` の delta を追加し、execution-oriented active listing と TUI read-only rejected row の責務分離を明文化する (verification: strict validate + spec scenario review)
- [x] Task 2: TUI change refresh で rejected marker-bearing change を read-only row として表示用に取り込む設計を追加する（`src/tui/state.rs`, `src/tui/runner.rs`, 必要なら表示用 helper） (verification: integration - TUI state tests で rejected change appears after refresh を追加)
- [x] Task 3: rejected row の表示 semantics を固定する（status=`rejected`, reducer vocabulary と整合する色、`selected=false` 維持） (verification: unit - TUI state tests で rejected row status/color/selection を確認)
- [x] Task 4: rejected row の操作ガードを追加する（Space / `@` / `F5` / resume 系操作で x マークや queue intent を付けられないようにする） (verification: integration - key handling / reducer sync tests で rejected row remains unselected を追加)
- [x] Task 5: marker removal による再活性化回帰を固定する（`REJECTED.md` 削除後は `not queued` かつ unselected で戻ることを確認する） (verification: integration - refresh reconciliation tests を追加)

## Future Work

- rejected reason を TUI detail popup や tooltip で見せる UX 改善
- terminal rows 全般の filter / grouping 表示の見直し
