## MODIFIED Requirements

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
