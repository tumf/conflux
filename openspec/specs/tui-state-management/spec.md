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

Running モードで change の queue-oriented 実行マーク操作に使う `display_status_cache` は、shared reducer display snapshot と `ChangesRefreshed` の往復後も queue 状態と active 状態を正しく反映しなければならない（MUST）。

Running モードで error 状態の change に Space キーを押した場合、retry mark の設定だけでなく、実際に queue への追加/削除コマンドを発行しなければならない。

<!-- Expected canonical result after archive: Running-mode queue toggle semantics will explicitly require reducer-synced display_status_cache so Space/x remain actionable after refresh. -->

#### Scenario: Space on error change marks for retry and adds to queue

**Given**: Running モードで display_status_cache が "error" の change が存在する
**When**: ユーザーが Space キーを押す
**Then**: change の selected が true になり、TuiCommand::AddToQueue が発行され、display_status_cache が "queued" に遷移する

#### Scenario: Space on retried error change clears mark and removes from queue

**Given**: Running モードで display_status_cache が "queued"（error からの遷移後）の change が存在する
**When**: ユーザーが Space キーを押す
**Then**: change の selected が false になり、TuiCommand::RemoveFromQueue が発行され、display_status_cache が "not queued" に遷移する

#### Scenario: Running new change remains queue-toggleable after refresh

**Given**: TUI が Running モードで、新しく検出された change の display_status_cache が "not queued" である
**When**: ユーザーが Space キーでその change を queue に追加し、その後 reducer display sync と `ChangesRefreshed` が発生する
**Then**: change は queue-oriented 実行マークを保持する
**And**: subsequent Running-mode queue controls still treat the row as actionable `queued` or `not queued` state rather than regressing it to an incorrect inactive status

#### Scenario: Running bulk toggle preserves actionable non-active rows

**Given**: TUI が Running モードで、eligible な `not queued` / `queued` row と active row が混在している
**When**: ユーザーが `x` で bulk toggle を実行する
**Then**: `not queued` / `queued` row には single-row Space と同じ queue add/remove semantics が適用される
**And**: active row は bulk toggle 対象として state change されない

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

Changes viewのbulk execution mark toggleは、操作開始時点のeligibleなproposal全体を1つの対象集合として扱わなければならない（SHALL）。対象集合に未マークが1件でもあれば対象全件をマークし、対象全件がマーク済みなら対象全件をアンマークしなければならない（SHALL）。

既存の安全guardによりactive、rejected、またはparallel-ineligibleなproposalは対象集合へ含めてはならない（MUST NOT）。除外行が存在する場合、TUIは操作された件数、除外された件数、およびユーザーが理解または対処できる除外理由を表示しなければならない（SHALL）。対象集合が空の場合も無反応にしてはならない（MUST NOT）。

Running modeのeligibleなqueue-mutating rowには、単一行のSpace操作と同じAddToQueue/RemoveFromQueue semanticsを適用しなければならない（SHALL）。active rowをbulk操作から停止要求へ変換してはならない（MUST NOT）。

#### Scenario: 部分的にマーク済みならeligible全件をマークする

**Given**: eligibleなproposalの一部だけが実行マーク済みである
**When**: ユーザーがChanges viewで`x`を押す
**Then**: eligibleなproposalはすべて実行マーク済みになる
**And**: 既にマーク済みのproposalもマーク状態を維持する

#### Scenario: eligible全件がマーク済みなら全件をアンマークする

**Given**: eligibleなproposalがすべて実行マーク済みである
**When**: ユーザーがChanges viewで`x`を押す
**Then**: eligibleなproposalはすべて未マークになる

#### Scenario: eligibleとineligibleが混在する

**Given**: eligibleな未マークproposalと、active、rejected、またはparallel-ineligibleなproposalが混在する
**When**: ユーザーがChanges viewで`x`を押す
**Then**: eligibleなproposalはすべてマークされる
**And**: ineligibleなproposalのマーク状態は変更されない
**And**: TUIは変更件数、除外件数、および除外理由を表示する

#### Scenario: bulk対象が存在しない

**Given**: 表示中のproposalがすべてbulk toggleの対象外である
**When**: ユーザーがChanges viewで`x`を押す
**Then**: proposalの状態は変更されない
**And**: TUIは対象がない理由を表示する

#### Scenario: Running modeでqueue commandを全対象へ発行する

**Given**: Running modeで複数のeligibleな`not queued` proposalが未マークであり、active proposalも存在する
**When**: ユーザーがChanges viewで`x`を押す
**Then**: 各eligibleな`not queued` proposalがマークされ、それぞれにAddToQueue commandが発行される
**And**: active proposalには停止commandもstate changeも発生しない
