
### Requirement: merge-attempt-resolve-priority

archive 完了後の merge 試行において、base dirty チェックよりも先にプロジェクトレベルの resolve 進行状況（`auto_resolve_count` + `manual_resolve_count`）を確認し、resolve が進行中であれば auto_resumable な MergeDeferred として扱う。

#### Scenario: archive-completed-while-another-change-resolving

**Given**: Change A が resolving 状態（auto_resolve_count > 0 または manual_resolve_count > 0）、Change B の archive が完了した
**When**: Change B の merge が attempt_merge() で試行される
**Then**: base dirty チェックの前に resolve 進行中を検出し、MergeAttempt::Deferred を返し、Change B は ResolveWait に遷移して resolve キューに追加される

#### Scenario: archive-completed-no-resolve-active

**Given**: resolve 進行中の change がない（auto_resolve_count == 0 かつ manual_resolve_count == 0）、Change B の archive が完了した
**When**: Change B の merge が attempt_merge() で試行される
**Then**: 従来通り base_dirty_reason() で判定され、dirty なら MergeDeferred、clean なら merge が実行される


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


### Requirement: is-dirty-reason-auto-resumable

**Reason**: dirty reason の文字列解析による auto_resumable 判定は、resolve 中の uncommitted changes を正しく分類できない。resolve カウンターによる判定に統一するため削除。


### Requirement: merge-attempt-resolve-priority

archive 完了後の merge 試行において、システムは resolve カウンター（`auto_resolve_count` + `manual_resolve_count`）を最優先でチェックしなければならない（MUST）。これは Project スコープ（同一 `OrchestratorState` 内）の resolve 進行状況である。resolve 進行中であれば auto_resumable な MergeDeferred として即座に返さなければならない（MUST）。resolve が進行中でなく base が dirty な場合は、reason の内容に関わらず常に `auto_resumable=false` の MergeDeferred としなければならない（MUST）。

#### Scenario: archive-completed-while-another-change-resolving

**Given**: 同一 Project 内で Change A が resolving 状態（auto_resolve_count > 0 または manual_resolve_count > 0）、Change B の archive が完了した
**When**: Change B の merge が attempt_merge() で試行される
**Then**: base dirty チェックの前に Project 内の resolve 進行中を検出し、MergeAttempt::Deferred("Resolve in progress") を返し、Change B は ResolveWait に遷移して resolve キューに追加される

#### Scenario: archive-completed-no-resolve-active-base-dirty

**Given**: 同一 Project 内で resolve 進行中の Change がない（auto_resolve_count == 0 かつ manual_resolve_count == 0）、Change B の archive が完了した、base branch に uncommitted changes がある
**When**: Change B の merge が attempt_merge() で試行される
**Then**: base_dirty_reason() で dirty を検出し、MergeDeferred(auto_resumable=false) を返し、Change B は MergeWait に遷移する

#### Scenario: archive-completed-no-resolve-active-base-clean

**Given**: 同一 Project 内で resolve 進行中の Change がない、base branch が clean
**When**: Change B の merge が attempt_merge() で試行される
**Then**: merge が実行される


### Requirement: merge-attempt-resolve-priority

archive 完了後の merge 試行において、システムは resolve カウンター（`auto_resolve_count` + `manual_resolve_count`）を最優先でチェックしなければならない（MUST）。これは Project スコープ（同一 `OrchestratorState` 内）の resolve 進行状況である。resolve 進行中であれば auto_resumable な MergeDeferred として即座に返さなければならない（MUST）。resolve が進行中でなく base が dirty な場合は、reason の内容に関わらず常に `auto_resumable=false` の MergeDeferred としなければならない（MUST）。

scheduler は spawn 済みの merge タスクが完了するまで終了してはならない（MUST NOT）。pending merge task が存在する間は、`queued`・`in_flight`・`resolve_wait`・`manual_resolve` がすべて空であっても scheduler loop を継続しなければならない（MUST）。

#### Scenario: archive-completed-while-another-change-resolving

**Given**: 同一 Project 内で Change A が resolving 状態（auto_resolve_count > 0 または manual_resolve_count > 0）、Change B の archive が完了した
**When**: Change B の merge が attempt_merge() で試行される
**Then**: base dirty チェックの前に Project 内の resolve 進行中を検出し、MergeAttempt::Deferred("Resolve in progress") を返し、Change B は ResolveWait に遷移して resolve キューに追加される

#### Scenario: archive-completed-no-resolve-active-base-dirty

**Given**: 同一 Project 内で resolve 進行中の Change がない（auto_resolve_count == 0 かつ manual_resolve_count == 0）、Change B の archive が完了した、base branch に uncommitted changes がある
**When**: Change B の merge が attempt_merge() で試行される
**Then**: base_dirty_reason() で dirty を検出し、MergeDeferred(auto_resumable=false) を返し、Change B は MergeWait に遷移する

#### Scenario: archive-completed-no-resolve-active-base-clean

**Given**: 同一 Project 内で resolve 進行中の Change がない、base branch が clean
**When**: Change B の merge が attempt_merge() で試行される
**Then**: merge が実行される

#### Scenario: single-change-scheduler-waits-for-merge

**Given**: 実行中の change が1つだけ、archive が正常に完了し spawn_merge_task が呼ばれた
**When**: scheduler loop の break 条件が評価される
**Then**: pending merge task が 0 になるまで scheduler は終了せず、merge task が完了して MergeCompleted が送信される


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
