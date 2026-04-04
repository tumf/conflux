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

Running モードで error 状態の change に Space キーを押した場合、retry mark の設定だけでなく、実際に queue への追加/削除コマンドを発行しなければならない。

#### Scenario: Space on error change marks for retry and adds to queue

**Given**: Running モードで display_status_cache が "error" の change が存在する
**When**: ユーザーが Space キーを押す
**Then**: change の selected が true になり、TuiCommand::AddToQueue が発行され、display_status_cache が "queued" に遷移する

#### Scenario: Space on retried error change clears mark and removes from queue

**Given**: Running モードで display_status_cache が "queued"（error からの遷移後）の change が存在する
**When**: ユーザーが Space キーを押す
**Then**: change の selected が false になり、TuiCommand::RemoveFromQueue が発行され、display_status_cache が "not queued" に遷移する

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
