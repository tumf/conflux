## Implementation Tasks

- [x] 1. rejecting recovery の tasks 更新先契約を `openspec/specs/parallel-execution/spec.md` と必要なら `openspec/specs/orchestration-state/spec.md` の delta に追加し、active change dir 不在時は archived workspace entry の `tasks.md` を canonical fallback とすることを明記する (verification: integration - `cflx openspec validate resume-rejecting-from-archived-worktree --strict --evidence warn` が成功し、delta が active/archived path resolution を記述する)
- [x] 2. `src/orchestration/rejection.rs` に recovery tasks path resolver を追加し、`append_recovery_task()` が `openspec/changes/<change_id>/tasks.md` と `openspec/changes/archive/<date>-<change_id>/tasks.md` の両方を探索して、現在存在する canonical tasks.md に追記できるようにする (verification: unit - rejection tests が active path あり/なしの両ケースで期待 path を選び、追記内容が重複しないことを確認する)
- [x] 3. `RESUME` と `BLOCK` の両フローで新 resolver を使い、path 解決失敗時の error に探索済み path 一覧を含める (verification: unit - rejection flow tests が archived workspace で error にならず、missing-both-path case では explored paths を含む message を返すことを確認する)
- [ ] 4. `src/parallel/dispatch.rs` または同等 integration test に archived workspace の rejection review follow-up regression を追加し、`WorkspaceState::Archived` の change が `REJECTION_REVIEW: RESUME` で `Applying`、`REJECTION_REVIEW: BLOCK` で `Blocked` に遷移できることを確認する (verification: integration - dispatch/reducer tests が archived change の resume/block outcome を通し、`No such file or directory` が再発しないことを確認する)
- [ ] 5. task progress / runner の既存 archive fallback と rejecting recovery path が整合することを確認し、必要ならコードコメントまたは helper 名で archived workspace support を明示する (verification: unit - `src/task_parser.rs` の archive fallback tests と `src/orchestration/rejection.rs` の新 resolver tests が同じ archive path precedence を前提に通り、責務境界を示すコメント差分が repository 上で確認できることを確認する)
- [x] 6. proposal delta と関連実装変更をまとめて検証する (verification: integration - `cflx openspec validate resume-rejecting-from-archived-worktree --strict --evidence warn`, `cargo test orchestration::rejection`, `cargo test parallel::tests::executor`, `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- rejecting recovery task を archived proposal/timeline UI から編集しやすくする operator UX
- archived workspace の blocker metadata を structured event として永続化する telemetry 拡張
