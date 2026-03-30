## MODIFIED Requirements

### Requirement: merge-attempt-resolve-priority

archive 完了後の merge 試行において、resolve カウンター（`auto_resolve_count` + `manual_resolve_count`）を最優先でチェックする。resolve 進行中であれば auto_resumable な MergeDeferred として即座に返す。resolve が進行中でなく base が dirty な場合は、reason の内容に関わらず常に `auto_resumable=false` の MergeDeferred とする。`is_dirty_reason_auto_resumable()` による reason 文字列解析は行わない。

#### Scenario: archive-completed-while-another-change-resolving

**Given**: Change A が resolving 状態（auto_resolve_count > 0 または manual_resolve_count > 0）、Change B の archive が完了した
**When**: Change B の merge が attempt_merge() で試行される
**Then**: base dirty チェックの前に resolve 進行中を検出し、MergeAttempt::Deferred("Resolve in progress") を返し、Change B は ResolveWait に遷移して resolve キューに追加される

#### Scenario: archive-completed-no-resolve-active-base-dirty

**Given**: resolve 進行中の change がない（auto_resolve_count == 0 かつ manual_resolve_count == 0）、Change B の archive が完了した、base branch に uncommitted changes がある
**When**: Change B の merge が attempt_merge() で試行される
**Then**: base_dirty_reason() で dirty を検出し、MergeDeferred(auto_resumable=false) を返し、Change B は MergeWait に遷移する（ユーザーによる手動 cleanup 待ち）

#### Scenario: archive-completed-no-resolve-active-base-clean

**Given**: resolve 進行中の change がない、base branch が clean
**When**: Change B の merge が attempt_merge() で試行される
**Then**: merge が実行される

## REMOVED Requirements

### Requirement: is-dirty-reason-auto-resumable

**Reason**: dirty reason の文字列解析による auto_resumable 判定は、resolve 中の uncommitted changes を正しく分類できない。resolve カウンターによる判定に統一するため削除。
