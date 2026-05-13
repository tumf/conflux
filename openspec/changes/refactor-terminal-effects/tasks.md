## Implementation Tasks

- [x] 1. Characterization: 現在の reducer command 副作用を固定するテストを確認・追加する。verification: unit - `cargo test orchestration::state` または該当状態テストで AddToQueue、ResolveMerge、DequeueChange、StopChange の `ReduceOutcome` と wait queue 更新を確認する。completion: コマンドごとの terminal/activity/wait/queue_intent の代表結果がテストで追跡可能になっている。

- [x] 2. Characterization: archive/merge/rejection 系 ExecutionEvent の副作用を固定するテストを確認・追加する。verification: unit - `cargo test orchestration::state` で ChangeArchived、MergeDeferred、MergeCompleted、RejectionReviewCompleted、ResolveFailed の代表経路を確認する。completion: resolve_wait_queue、reject_wait_queue、blocked metadata の追加・削除がテストで明示されている。

- [x] 3. wait queue 操作を内部 helper に抽出する。verification: unit - `cargo test orchestration::state` が成功し、resolve/reject queue の retain/push 重複が helper 経由に整理されている。completion: 同じ queue cleanup 操作が複数 match arm に散らばらず、意図名を持つ helper から実行されている。

- [x] 4. terminal/activity/wait transition の共通処理を内部 helper に抽出する。verification: unit - `cargo test orchestration::state` で `invariants_hold` / `global_invariants_hold` を含む既存テストが成功する。completion: terminal 化、stalled 化、success completion の代表処理が helper 化され、公開挙動は変わっていない。

- [x] 5. 最終回帰確認を実行する。verification: integration - `cargo fmt --check` と `cargo test` が成功する。completion: 既定テストスイートが成功し、CLI/TUI/Web の仕様上の状態表示変更がないことを確認している。

## Future Work

`src/orchestration/state.rs` のファイル分割は、helper 抽出後に別提案として扱う。

## Final Validation

実装後の OpenSpec 最終確認は `cflx openspec validate refactor-terminal-effects --strict` を使用する。
