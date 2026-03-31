## MODIFIED Requirements

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
