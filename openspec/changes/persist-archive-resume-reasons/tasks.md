## Implementation Tasks

- [x] 1. `src/history.rs`, `src/agent/history_ops.rs`, `src/orchestration/archive.rs`, `src/parallel/executor.rs` の archive attempt model を整理し、primary archive failure reason を enum / structured field で表現できるようにする (verification: unit - archive history formatting / attempt recording tests が verification failure, prerequisite blocker, command failure, stall を別 reason として保持することを確認する)
- [x] 2. worktree 外 durable archive resume state を追加し、保存先・読込 helper・最低保持項目（change_id, revision, attempt, status, primary_reason, summary, updated_at）を `src/orchestration` または `src/parallel` 配下へ実装する (verification: unit - state roundtrip tests が保存/復元/削除と revision mismatch handling を確認する)
- [x] 3. `src/parallel/execution/state.rs` 相当の resume detection と `src/parallel/dispatch.rs` の resume routing を更新し、`Archiving`/`Archived` 判定時に durable archive state を参照して reason-aware handoff / retry context を生成する。ただし `Archived` terminal merge handoff の挙動は維持する (verification: integration - workspace resume tests が `Archiving` resume で previous archive reason を復元し、`Archived` workspace は引き続き merge handoff のみへ進むことを確認する)
- [x] 4. `src/parallel/executor.rs`, `src/orchestration/archive.rs`, `src/events.rs` の archive retry / resume / failure event を更新し、generic retry wording ではなく structured reason と summary を downstream へ渡せるようにする (verification: integration - executor / event bridge tests が archive retry scheduled, archive resumed, archive failed の各経路で reason-aware log/event payload を確認する)
- [x] 5. resume/再起動後の最初の archive retry でも prior reason が agent prompt または equivalent runtime context に復元されるよう、archive history 復元または durable state injection を実装する (verification: unit/integration - simulated restart test が prior archive reason を失わず次回 archive prompt/context に含めることを確認する)
- [x] 6. canonical spec / design / log wording を同期し、existing active change `align-archive-readiness-failure-reporting` と矛盾しない archive reason taxonomy と resume observability を明文化する (verification: manual - proposal/design/spec と related active change を見比べ、root-cause surfacing と durable persistence の責務境界が重複せず補完関係になっていることをレビューする)
- [x] 7. full verification を実行し、archive reason persistence 導入後も archive loop / resume regression がないことを確認する (verification: integration - `cflx openspec validate persist-archive-resume-reasons --strict --evidence warn`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`)

## Future Work

- archive reason を dashboard 上で badge / timeline としてどう見せるかの最終 UX は別 change で polish する
- apply / acceptance / resolve にも同じ durable failure-reason protocol を広げる場合は別 proposal で共通化する

## Acceptance #1 Failure Follow-up
- [x] src/parallel/dispatch.rs:593-607 の ArchiveResumed 発火と src/parallel/dispatch.rs:1678-1694 の ArchiveFailed 発火が、durable archive state を load_archive_state(&workspace.path) + change_id 一致だけで参照しており、src/parallel/archive_state.rs:86-100 に実装済みの revision guard (load_archive_state_matching) を使っていません。workspace path が再利用されて古い revision の archive state が残った場合、resume/failure event が現行 revision とは無関係な stale reason/summary を公開し、spec の current workspace resume/failure reason 復元要件を破ります。dispatch 側でも current revision を解決して revision 一致の state のみを使うよう修正が必要です。
- [x] 受け入れコマンドの必須チェックである git status --porcelain が空であることを確認し、実ワークツリーが clean であることを再確認しました。archive commit 実行を妨げる未コミット変更は現時点で存在しません。

## Acceptance #2 Failure Follow-up
- [x] src/parallel/dispatch.rs:596-604 の ArchiveResumed 経路と src/parallel/dispatch.rs:1691-1700 の ArchiveFailed 経路は、get_current_commit(&workspace.path) に失敗した場合に load_archive_state(&workspace.path) + change_id 一致へフォールバックしています。これだと前回指摘と同じく revision guard なしで durable archive state を採用してしまい、workspace path 再利用時や revision 解決失敗時に stale な reason / summary を現行 workspace の resume/failure event として公開できます。前回 finding が求めていた『current revision に対応する state のみを使う』条件をまだ満たしていないため、revision 未解決時は event reason/summary を出さないか、別の手段で現行 revision 一致を確認してから state を使うよう修正が必要です。
