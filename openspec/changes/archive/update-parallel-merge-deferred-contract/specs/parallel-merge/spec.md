## MODIFIED Requirements

### Requirement: merge-attempt-resolve-priority

archive 完了後の merge 試行において、システムは resolve カウンター（`auto_resolve_count` + `manual_resolve_count`）を最優先でチェックしなければならない（MUST）。これは同一 Project / 同一 parallel orchestration scope 内の resolve 進行状況である。

resolve が進行中であれば、merge 試行は auto-resumable な deferred として即座に返されなければならない（MUST）。

resolve が進行中でなく base branch が dirty な場合は、reason の内容に関わらず常に manual intervention が必要な deferred として扱わなければならない（MUST）。dirty reason の文字列解析によって auto-resumable 判定を行ってはならない（MUST NOT）。

scheduler は spawn 済みの background merge task が残っている間、`queued`・`in_flight`・`resolve_wait`・`manual_resolve` が空でも終了してはならない（MUST NOT）。

#### Scenario: archive-completed-while-another-change-resolving

**Given**: 同一 Project 内で Change A が resolving 状態（`auto_resolve_count > 0` または `manual_resolve_count > 0`）であり、Change B の archive が完了している
**When**: Change B の merge が `attempt_merge()` で試行される
**Then**: base dirty チェックより先に resolve 進行中が検出される
**And**: merge は auto-resumable deferred として返される

#### Scenario: archive-completed-no-resolve-active-base-dirty

**Given**: 同一 Project 内で resolve 進行中の change がなく、Change B の archive が完了しており、base branch に uncommitted changes がある
**When**: Change B の merge が `attempt_merge()` で試行される
**Then**: merge は manual intervention が必要な deferred として返される
**And**: auto-resumable 判定は dirty reason の文字列内容に依存しない

#### Scenario: scheduler-waits-for-background-merge-tasks

**Given**: `queued`、`in_flight`、`resolve_wait`、`manual_resolve` がすべて空である
**And**: background merge task を表す `pending_merge_count` が 1 以上である
**When**: scheduler loop が終了条件を評価する
**Then**: scheduler は終了しない
**And**: background merge task の完了処理を待つ
