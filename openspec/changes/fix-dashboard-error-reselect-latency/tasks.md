## Implementation Tasks

- [x] 1. `src/server/api/control.rs` の single-change / bulk toggle 成功経路に、dashboard が full snapshot 待ちせず selection 変化を受け取れる差分反映イベントまたは同等の即時更新メカニズムを追加する (verification: integration - add or extend `src/server/api/control.rs` / `src/server/api/ws.rs` tests proving `POST /api/v1/projects/{id}/changes/{change_id}/toggle` emits an immediate selection update payload without waiting for the next `full_state` cycle)
- [x] 2. `src/server/api/ws.rs` と関連 snapshot/build 経路を調整し、error change の `status = error` を維持したまま `selected` だけが再選択結果へ更新されること、および rejected row が read-only semantics を保つことを固定する (verification: integration - add or extend `src/server/api/ws.rs` tests proving error rows can become `selected = true` while staying `status = error`, and rejected rows remain `selected = false`)
- [x] 3. `dashboard/src/store/useAppStore.ts` と selection 更新フローに optimistic toggle state を保持できる action を追加し、explicit toggle 直後に checkbox を即時反映できるようにする (verification: unit - extend `dashboard/src/store/useAppStore.test.ts` and/or `dashboard/src/components/ChangeRow.test.tsx` to prove checkbox state flips immediately on user toggle before a later `full_state` refresh)
- [x] 4. `dashboard/src/components/ChangeRow.tsx` と関連 API 呼び出しを更新し、toggle failure 時には optimistic state を rollback してユーザーに失敗を通知する (verification: unit - extend `dashboard/src/components/ChangeRow.test.tsx` to prove failed toggle restores the prior selection state and surfaces an error notification)
- [x] 5. `dashboard/src/api/wsClient.ts` と関連 client/state 同期経路を更新し、server 差分 update と optimistic state が競合せず最終確定値へ収束するようにする (verification: unit - add `dashboard/src/api/wsClient.test.ts` or equivalent client-sync test covering server-pushed selection updates reconciling correctly with optimistic error-row state)
- [x] 6. bulk toggle でも error row を含む selection 表示が即時に更新され、次回 full-state/poll を待たず一貫した UI になることを固定する (verification: unit/integration - extend `dashboard/src/components/ChangeRow.test.tsx` and relevant Rust API tests so toggle-all updates visible selection state immediately, including previously unselected error rows)
- [x] 7. proposal delta と関連実装の検証を完了する (verification: integration - run `cflx openspec validate fix-dashboard-error-reselect-latency --strict --evidence warn`, `cargo test`, `cargo clippy --all-targets --all-features -- -D warnings`, `npm test`, and `npm run lint` in `dashboard/`)

## Future Work

- multi-client 同時閲覧時の selection conflict policy を必要なら明文化する
- change row 差分更新イベントの payload 契約を他の per-row UI action に拡張するか検討する

## Acceptance #1 Failure Follow-up

- [x] `dashboard/src/store/useAppStore.ts` の `CLEAR_OPTIMISTIC_CHANGE_SELECTION` は optimistic フラグを消すだけで `APPLY_CHANGE_UPDATE` で受け取った server 確定値を `projects[*].changes[*].selected` に反映していないため、競合時に UI が optimistic 値のまま残る。server 確定値へ収束する state 遷移と、その差分を再現する unit test を追加する。
- [x] `openspec/changes/fix-dashboard-error-reselect-latency/tasks.md` の Task 6 の検証対象は「bulk toggle API が full-state/poll を待たず selection を更新すること」であり、現行 dashboard UI で `toggleAllChangeSelection()` が未参照でも不整合ではない。Task 6 の受け入れ観点を backend/API 経路（Rust integration + 既存 frontend selection 同期）として扱うことを明記し、誤解を招く「dashboard UI からの呼び出し必須」解釈を修正する。

## Acceptance #2 Failure Follow-up
- [x] `dashboard/src/api/restClient.ts:208-214` の `toggleAllChangeSelection()` は依然として dashboard UI から参照されておらず、`dashboard/src/App.tsx:617-628,744-755` と `dashboard/src/components/ChangeRow.tsx:68-87` でも individual toggle しか扱っていない。`openspec/changes/fix-dashboard-error-reselect-latency/specs/web-monitoring/spec.md:30-34` と `proposal.md:48` は bulk selection toggle をユーザーが起動した際の即時 UI 更新を要求しているため、bulk toggle の実 UI 経路とその optimistic 即時反映/rollback の検証を追加するか、proposal/spec を正式に変更する必要がある。
