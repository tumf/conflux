## Why

現状の archive loop は、同一プロセス中の retry では `ArchiveHistory` を使って previous failure を prompt へ渡せるが、resume/再起動をまたぐ durability がない。これにより、runtime が再開したときには file state から `Archiving` / `Archived` を推定できても、「なぜ前回 archive が止まったか」を lossless に説明できない。

その結果、ユーザーや後続 agent からは「前回の失敗が引き継がれていない」「なぜ再ループしたか分からない」と見えやすい。`align-archive-readiness-failure-reporting` が扱う root-cause surfacing を、resume persistence と structured event reason に延長する必要がある。

## Design Goals

- archive retry / resume の primary reason を同一ラン内・resume 後・UI 表示で一貫して扱う
- `Archived` terminal handoff の既存保証を壊さない
- file-state based archive verification を維持しつつ、reason persistence を追加する
- 将来 apply/acceptance/resolve へ拡張可能な最小 durable state 形を採る

## Non-Goals

- retry 回数や stall detector policy 自体の大規模 redesign
- archive 以外の operation failure persistence を同 proposal で統一
- dashboard final UX の作り込み

## Proposed Design

### 1. Archive primary reason taxonomy

archive attempt の primary reason を少なくとも以下で区別する。

1. `command_failed`
2. `prerequisite_blocker`
3. `verification_failed`
4. `post_archive_completion_failed`
5. `stalled`
6. `resumed_context_only`（resume 時に直前 reason を再提示するための synthesized context）

`verification_failed` は file-state symptom を表し、`prerequisite_blocker` や `post_archive_completion_failed` が既知ならそれを主原因として優先する。

### 2. Durable archive resume state

acceptance-state と同様に worktree 外の durable state を持つ。

最低保持項目:

- `change_id`
- `revision`
- `attempt`
- `status` (`running|failed|stalled|passed`)
- `primary_reason`
- `summary`
- `updated_at`

保存タイミング:

- archive 開始時: `running`
- archive retry reason 確定時: `failed` + reason/summary
- stall 確定時: `stalled`
- archive fully complete / terminal handoff 時: clear または `passed` 後に cleanup

### 3. Resume integration

resume detection は file state を primary に維持する。ただし `WorkspaceState::Archiving` もしくは archive retry continuation を選ぶ際には durable archive state を追加参照する。

- `Archived`: 既存どおり terminal merge handoff。durable state は optional supplemental context のみ
- `Archiving`: previous archive reason を resume context として復元し、next archive attempt / event / log に渡す
- state と revision が不整合な場合: file state を優先しつつ stale durable state を無視または clear する

### 4. History restoration vs. state injection

再起動後に prior reason を agent に渡す経路は 2 案ある。

- A. durable state から `ArchiveHistory` を再構築する
- B. archive prompt 構築時に durable state を別 context block として注入する

本 proposal では B を優先する。理由は:

- 既存 in-memory history semantics を壊しにくい
- durable state と in-memory attempt list の責務を混ぜにくい
- resume 後最初の 1 回だけ prior reason を注入できれば主要 UX を満たせる

必要なら将来 A に拡張できるよう field naming を合わせる。

### 5. Event / log surfacing

archive retry / resume / failure の event payload に reason と summary を持たせる。

最低限ほしい観測点:

- retry scheduled: why retrying
- archive resumed: why resuming archive instead of fresh apply/acceptance
- archive failed terminally: primary reason + supplemental file-state symptom

これにより TUI/Web は generic な `retrying archive command` のみでなく、`verification failed: change dir still exists` や `prerequisite blocker: durable acceptance-pass state missing` 等を表示できる。

## Verification Strategy

- archive state roundtrip / stale-state handling の unit test を追加する
- workspace resume tests に `Archiving` + prior durable reason の case を追加する
- `Archived` terminal merge handoff regression test を維持する
- simulated restart test で prior reason が archive prompt/context に復元されることを確認する
- `cflx openspec validate ... --strict --evidence warn` と Rust test/lint を通す
