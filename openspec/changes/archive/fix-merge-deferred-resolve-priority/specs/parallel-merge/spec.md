## MODIFIED Requirements

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
