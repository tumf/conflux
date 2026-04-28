---
change_type: implementation
priority: high
dependencies: []
references:
  - src/server/api/control.rs
  - src/server/api/ws.rs
  - src/server/registry.rs
  - dashboard/src/components/ChangeRow.tsx
  - dashboard/src/api/wsClient.ts
  - dashboard/src/store/useAppStore.ts
  - openspec/specs/server-api/spec.md
  - openspec/specs/web-monitoring/spec.md
---

# Change: fix dashboard error reselect latency

**Change Type**: implementation

## Premise / Context

- 現セッションでは、error 状態になった change / proposal row に再度 x を付けても dashboard 上ですぐ反映されず、しばらく遅れて checked に見えることが問題として報告された。
- 調査した実装では、server の toggle API は即時に `selected` を反転するが、dashboard は toggle 後にローカル state を更新せず、次の `full_state` 反映待ちになっている。
- canonical spec では `POST /api/v1/projects/{id}/changes/{change_id}/toggle` 後に `change_update` が配信される想定があるが、現状の dashboard WebSocket client はその差分経路を有効活用していない。
- 既存 semantics として、error change は error 化時に `selected = false` となり、再度 mark すると次回 Run の対象へ戻る。この semantics 自体は維持すべきである。

## Problem / Context

dashboard で error change を再選択して retry 対象へ戻したい場面でも、checkbox の見た目が即時に変わらず、ユーザーには操作が効いていないように見える。特に error row は通常 row より操作意図が明確である必要があり、retry mark の反映遅延は UX 上の混乱を生む。

この遅延は backend の selection state 更新ではなく、frontend が explicit toggle の結果を即時表示しないこと、および server/client 間の差分反映経路が弱いことに起因する。結果として UI は次回 `full_state` または refresh タイミングまで古い `selected` 表示を保持してしまう。

## Proposed Solution

error change の再選択を含む dashboard の change selection 操作を、ユーザー操作直後に即時反映されるフローへ揃える。

- server の single-change / bulk selection toggle 成功時に、次回 full snapshot 待ちではない即時反映経路を提供する
- dashboard は explicit selection toggle 成功を待たずに optimistic に checkbox 表示を更新できるようにし、失敗時のみ rollback する
- server からの差分更新を dashboard store が適用し、optimistic state と最終確定値を整合させる
- error row の `status = error` を保ったまま `selected = true` へ戻る既存 semantics を維持する
- rejected terminal row や active execution candidate 判定など、selection 以外の既存 semantics は変更しない

## Acceptance Criteria

- dashboard で `status = error` かつ `selected = false` の row をユーザーが toggle したとき、checkbox は次回 periodic / full snapshot を待たずに即時 checked 表示へ遷移する
- 上記 toggle 成功後、server 側の確定 state と dashboard 表示は一致し、error row は `status = error` を維持したまま `selected = true` として次回 Run の対象になる
- toggle API が失敗した場合、dashboard は optimistic selection を元に戻し、失敗をユーザーへ通知する
- individual toggle だけでなく、同じ change selection 反映経路を使う bulk toggle でも full snapshot 待ちの見た目遅延を残さない
- rejected row の read-only semantics、error row の retry semantics、global Run の selected-based targeting には回帰を入れない

## Explicit Completion Conditions

- `src/server/api/control.rs` と関連 WebSocket/state 配信経路に、selection toggle 成功直後の差分反映または同等の即時更新メカニズムが追加されている
- dashboard frontend に、explicit toggle 時の optimistic selection 更新と failure rollback が追加されている
- dashboard WebSocket/store 層が server からの差分 selection update を適用し、次回 `full_state` 待ちに依存せず整合する
- error row の再選択即時反映、toggle failure rollback、bulk toggle の反映、rejected row 非対象維持を確認する frontend / Rust tests が追加または更新されている
- proposal delta が `cflx openspec validate fix-dashboard-error-reselect-latency --strict` を通過する
- 関連検証コマンドとして少なくとも Rust 側テスト / lint 相当と dashboard 側 test / lint が成功する

## Out of Scope

- proposal session chat UI / proposal session backend の変更
- change list 全体のデザイン刷新
- error retry semantics 自体の再設計
- polling interval や full snapshot 配信頻度だけで遅延を誤魔化す変更
