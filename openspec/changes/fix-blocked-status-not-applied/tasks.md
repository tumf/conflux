## Implementation Tasks

- [x] 1. `src/parallel/queue_state.rs` の dependency block 反映を修正する: analyzer が返した `analysis_result.dependencies` に未解決 dependency がある change について、dispatch 対象に選ばれない場合でも `DependencyBlocked` / `DependencyResolved` が正しく発火するよう scheduler 選定ロジックを整理する (verification: queue_state unit test で analyzer dependency が未解決の change に blocked event が出ることを確認)
- [x] 2. blocked regression を reducer レベルで固定する: `src/orchestration/state.rs` の reducer test に analyzer dependency 由来の `DependencyBlocked` / `DependencyResolved` シナリオを追加し、display status が `blocked` → `queued/not queued` に遷移することを確認する (verification: reducer test が pass する)
- [x] 3. slot 上限下でも blocked が失われないことを検証する: available slot が先に埋まる order でも後続 dependency change が blocked として観測されるテストを `src/parallel/tests` か `src/parallel/queue_state.rs` 近傍テストへ追加する (verification: regression test が pass する)
- [x] 4. TUI / Web の状態収束を確認する: dependency block / resolve イベントを受けた後、TUI と Web 表示が `blocked` / 通常 queue 状態へ収束することを既存 state test に追加する (verification: 関連 state tests が pass する)
- [x] 5. spec delta を追加し strict validate を通す: parallel execution / orchestration state の dependency-blocked 契約を spec delta に明記し、`python3 /Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py validate fix-blocked-status-not-applied --strict` を pass させる (verification: strict validate が pass する)

## Future Work

- 回帰原因となった commit / refactor の事後分析とポストモーテム
- dependency block event の observability 強化（ログや debug counters の追加）
