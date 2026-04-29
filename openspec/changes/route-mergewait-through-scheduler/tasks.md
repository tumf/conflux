## Implementation Tasks

- [x] 1. `parallel-execution` と `orchestration-state` の delta に、`M` が direct execution ではなく scheduler-owned retry intent である canonical rule を追加する (verification: integration - `cflx openspec validate route-mergewait-through-scheduler --strict --evidence warn` が成功し、delta が intent ownership・execution ownership・completion semantics を明示する)
- [x] 2. `src/tui/command_handlers.rs` の `TuiCommand::ResolveMerge` 処理を reducer/shared-state intent 記録 + scheduler wakeup のみに変更し、direct `resolve_deferred_merge(...)` spawn を除去する (verification: unit - command handler / state tests が `ResolveMerge` 後に scheduler-visible intent だけが更新され、handler から direct execution を開始しないことを確認する)
- [x] 3. `src/parallel/orchestration.rs` と `src/parallel/queue_state.rs` に reducer-observable merge-wait retry intent の評価を追加し、通常 scheduler loop が merge / resolve retry を開始できるようにする (verification: integration - scheduler tests が `MergeWait` change へ retry intent を与えると通常 loop から retry が開始されることを確認する)
- [x] 4. `src/orchestration/state.rs` と関連 TUI state/reducer を更新し、`ResolveWait` / queued resolve intent の clear・completion・cancel が scheduler completion semantics と整合するようにする (verification: unit - reducer tests が manual resolve completion 後に queued resolve wait を clear し、refresh で `resolve pending` に戻らないことを確認する)
- [x] 5. `src/tui/queue.rs` と scheduler wakeup path を見直し、manual resolve 完了・retry eligible change・queue addition が同一 scheduler 再評価条件で扱われるようにする (verification: integration - resolve completion 相当の wakeup が debounce/flag drift で取りこぼされず、queue notification だけに依存しないことを確認する)
- [x] 6. `Resolving` 1件 + 空き slot あり + 別 change queue 追加 の回帰テストを追加し、`merge_wait` retry intent の存在下でも queued change の analysis / dispatch が通常どおり進むことを確認する (verification: integration - `src/parallel/tests/executor.rs` または同等テストで free-slot case が analysis / dispatch されることを確認する)
- [x] 7. `M` 押下後の `MergeWait` / `ResolveWait` / `Resolving` / `Merged` 表示と log wording を見直し、UI が scheduler-owned retry intent モデルと一致することを確認する (verification: unit - TUI state/event tests が `resolve pending` と `resolving` の遷移、および direct execution 前提でない log wording を確認する)
- [x] 8. proposal delta と実装変更をまとめて検証する (verification: integration - `cflx openspec validate route-mergewait-through-scheduler --strict --evidence warn`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- reducer-owned retry intent を dashboard / Web UI でも操作できるようにする
- queued resolve / merge retry の可視化を timeline 上で改善する
