## MODIFIED Requirements

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
