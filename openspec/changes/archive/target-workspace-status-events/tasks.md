## Implementation Tasks

- [x] `ExecutionEvent::WorkspaceStatusUpdated` に `change_id` を追加するか、change 単位の新イベントへ置き換える (verification: `cargo test --lib orchestration::state`)
- [x] `src/orchestration/state.rs` L962-988 の `current_change_id` 依存ロジックを削除し、イベント対象の change に直接 `ActivityState` を適用する (verification: reducer unit test)
- [x] `src/parallel/dispatch.rs` など WorkspaceStatus 更新を送出する箇所で、必ず対象 change_id を添えてイベントを発行する (verification: `cargo test --lib parallel::dispatch`)
- [x] 複数 change が同時に active な場合でも `rejecting` / `accepting` / `archiving` / `resolving` が別 change に誤適用されないテストを追加する (verification: `cargo test --lib orchestration::state`)
- [x] `WorkspaceStatusUpdated` と専用開始イベント (`ApplyStarted` など) の責務境界を整理し、active stage の正典が専用イベントであることをドキュメント化する (verification: spec delta / unit tests)

## Future Work

- `WorkspaceStatusUpdated` の完全削除と専用イベント一本化
