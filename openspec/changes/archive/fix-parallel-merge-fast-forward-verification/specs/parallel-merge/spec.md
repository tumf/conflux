## MODIFIED Requirements

### Requirement: merge-attempt-resolve-priority

archive 完了後の parallel merge 最終検証は、`Merge change: <change_id>` という merge commit subject が存在しない場合でも、対象 change が fast-forward により base branch へ統合済みなら成功として扱わなければならない（MUST）。

#### Scenario: parallel merge verification accepts fast-forward integration

**Given** archive が完了した change が parallel merge 経路で base branch に fast-forward されている
**When** `verify_merge_commits()` が merge 完了を検証する
**Then** merge commit subject が存在しなくても検証は成功する
**And** change は merge error にならない

#### Scenario: missing merge commit error only applies to unintegrated change

**Given** archive が完了した change に `Merge change: <change_id>` という merge commit subject が存在しない
**And** 対象 change は base branch にも統合されていない
**When** `verify_merge_commits()` が merge 完了を検証する
**Then** `Missing merge commit message containing change_id(s)` エラーを返してよい
