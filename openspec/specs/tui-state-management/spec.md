## Requirements

### Requirement: resolve-merge-reducer-sync

When a user triggers merge resolve (`M` key) on a `MergeWait` change, the shared orchestration reducer MUST be updated with `ResolveMerge` intent regardless of whether resolve executes immediately or is queued.

#### Scenario: immediate-resolve-syncs-reducer

**Given**: A change is in `MergeWait` state and no other resolve is in progress (`is_resolving == false`)
**When**: The user presses `M` to trigger resolve
**Then**: The shared reducer transitions the change to `ResolveWait`, and subsequent `ChangesRefreshed` display syncs preserve `ResolveWait` (not regress to `MergeWait`)

#### Scenario: queued-resolve-syncs-reducer

**Given**: A change is in `MergeWait` state and another resolve is already in progress (`is_resolving == true`)
**When**: The user presses `M` to queue resolve
**Then**: The shared reducer transitions the change to `ResolveWait`, and subsequent `ChangesRefreshed` display syncs preserve `ResolveWait`

### Requirement: error-change-space-toggle-running-mode

Running モードで change の表示に使う `display_status_cache` は、shared reducer display snapshot と `ChangesRefreshed` の往復後も queue 状態と active 状態を正しく反映しなければならない（MUST）。

Running モードで error 状態または error から遷移した queued 状態の change に Space を押した場合、Space は process-local execution mark のみを変更しなければならない（SHALL）。`AddToQueue`、`RemoveFromQueue`、retry、stop、scheduler、または display-status transition を発行してはならない（MUST NOT）。Retry execution は configured Start/F5 の final admission が current eligibility に基づいて決定しなければならない（SHALL）。

#### Scenario: Space on error change marks future retry intent only

**Given**: Running モードで display_status_cache が `error` の non-terminal change が存在する
**When**: ユーザーが Space を押す
**Then**: change の execution mark だけが toggle される
**And**: `AddToQueue`、`RemoveFromQueue`、retry、scheduler、mode、または display-status effect は発行されない

#### Scenario: Space on queued recovery row does not remove current work

**Given**: Running モードで error recovery 由来の `queued` change が存在する
**When**: ユーザーが Space を押す
**Then**: execution mark だけが toggle される
**And**: current-run queue と display status は変化しない

#### Scenario: Running new change remains mark-toggleable after refresh

**Given**: TUI が Running モードで、新しく検出された change の display_status_cache が `not queued` である
**When**: ユーザーが Space で mark を変更し、その後 reducer display sync と `ChangesRefreshed` が発生する
**Then**: frontend mark は shared ExecutionMarkStore の projection と一致する
**And**: refreshed display status does not create or remove queue intent from the mark

#### Scenario: Running bulk toggle preserves queue state

**Given**: TUI が Running モードで、not queued、queued、active、error、および wait rows が混在している
**When**: ユーザーが `x` で bulk toggle を実行する
**Then**: non-terminal rows receive the common execution-mark state
**And**: no row receives AddToQueue, RemoveFromQueue, stop, retry, or display-status mutation

### Requirement: update-change-status-guard-allows-error-to-queued

update_change_status のガードは "archived" と "merged" からの queued/not queued 遷移をブロックするが、"error" からの遷移は許可しなければならない。

#### Scenario: error to queued transition is allowed

**Given**: display_status_cache が "error" の change が存在する
**When**: update_change_status で next="queued" が呼ばれる
**Then**: ステータスが "queued" に更新される

#### Scenario: archived to queued transition is still blocked

**Given**: display_status_cache が "archived" の change が存在する
**When**: update_change_status で next="queued" が呼ばれる
**Then**: ステータスは変更されない（ガードでブロック）

### Requirement: resolve-merge-reducer-sync

When a user triggers merge resolve (`M` key) on a `MergeWait` change, the shared orchestration reducer MUST be updated with `ResolveMerge` intent regardless of whether resolve executes immediately or is queued.

モジュール分割後も、resolve 処理のイベントハンドラは `state/event_handlers/completion.rs` に配置され、既存の挙動を維持しなければならない (SHALL)。

#### Scenario: リファクタリング後も resolve-merge 動作が維持される

- **GIVEN** TUI イベントハンドラが `state/event_handlers/` に分割済みである
- **WHEN** change が `MergeWait` で `M` キーを押下する
- **THEN** 分割前と同一の reducer 更新と ResolveWait 遷移が行われる

### Requirement: Bulk execution mark toggle reports complete target results

Changes view の bulk execution mark toggle は、操作開始時点の visible non-terminal proposal 全体を1つの対象集合として扱わなければならない（SHALL）。対象集合に未マークが1件でもあれば対象全件をマークし、対象全件がマーク済みなら対象全件をアンマークしなければならない（SHALL）。

Execution mode、active/retry/wait status、Apply iteration-limit evidence、および現在の parallel eligibility は対象集合から non-terminal proposal を除外する理由にしてはならない（MUST NOT）。Archived、merged、pushed、および rejected proposal は対象集合へ含めてはならない（MUST NOT）。Terminal row の除外だけを理由に warning を表示してはならない（MUST NOT）。

Bulk 操作は execution mark のみを変更しなければならない（SHALL）。Running mode を含め、`AddToQueue`、`RemoveFromQueue`、stop/dequeue、retry、resolve、cancellation、scheduler、hook、または process-mode effect を発行してはならない（MUST NOT）。対象が存在する場合、結果は変更件数と共通 target mark state を報告しなければならない（SHALL）。表示中の proposal が terminal rows のみの場合は silent no-op としなければならない（SHALL）。Changes list 自体が空など terminal 除外以外の理由で対象集合が空の場合は、対象がない理由を報告しなければならない（SHALL）。

#### Scenario: 部分的にマーク済みなら全 non-terminal proposal をマークする

**Given**: visible non-terminal proposal の一部だけが実行マーク済みである
**When**: ユーザーが Changes view で `x` を押す
**Then**: visible non-terminal proposal はすべて実行マーク済みになる
**And**: 既にマーク済みの proposal もマーク状態を維持する

#### Scenario: 全 non-terminal proposal がマーク済みなら全件をアンマークする

**Given**: visible non-terminal proposal がすべて実行マーク済みである
**When**: ユーザーが Changes view で `x` を押す
**Then**: visible non-terminal proposal はすべて未マークになる
**And**: current-run queue と active execution は変化しない

#### Scenario: lifecycle と eligibility が混在しても同じ mark target になる

**Given**: active、error、wait、Apply-limit、parallel-ineligible、および ordinary non-terminal proposal が混在する
**When**: ユーザーが Changes view で `x` を押す
**Then**: 全 visible non-terminal proposal に同じ target mark state が適用される
**And**: queue、runtime、retry、resolve、cancellation、scheduler、hook、および mode state は変化しない

#### Scenario: terminal rows は bulk 対象外

**Given**: non-terminal proposal と archived、merged、pushed、または rejected proposal が混在する
**When**: ユーザーが Changes view で `x` を押す
**Then**: non-terminal proposal だけに共通 target mark state が適用される
**And**: terminal row exclusion warning は表示されない

#### Scenario: terminal rows だけの場合は silent no-op

**Given**: 表示中の proposal がすべて archived、merged、pushed、または rejected である
**When**: ユーザーが Changes view で `x` を押す
**Then**: proposal の状態は変更されない
**And**: mark refusal warning または empty-target message は表示されない

#### Scenario: Changes list が空なら理由を表示する

**Given**: Changes list に proposal が存在しない
**When**: ユーザーが Changes view で `x` を押す
**Then**: proposal の状態は変更されない
**And**: TUI は対象がない理由を表示する

<!-- Expected canonical result after archive: `tui-state-management` will retain refresh correctness while making single and bulk Space/x mark-only, excluding terminal rows, and distinguishing terminal-only silent no-op from other empty-target feedback. -->
