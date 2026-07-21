## ADDED Requirements

### Requirement: Obsolete selection implementation is not retained as an active module

到達不能で `SerialRunService` へ移行済みのchange selection実装は、active orchestration moduleとして保持してはならない。削除後も現役のserialおよびparallel selection contractを変更してはならない。

#### Scenario: Active serial selection remains owned by SerialRunService

**Given**: complete、incomplete、stalled、dependency-blockedなchangesがある
**When**: serial orchestratorが次のchangeを選択する
**Then**: 選択は `SerialRunService` の現役経路を通る
**And**: 優先順位と除外条件は削除前と同等である

#### Scenario: Removed module has no remaining references

**Given**: 旧 `orchestration::selection` moduleがproductionから参照されていない
**When**: moduleとmodule登録を削除する
**Then**: all-feature compilationは成功する
**And**: orphaned import、module declaration、dead-code suppressionは残らない

#### Scenario: Parallel selection remains unchanged

**Given**: parallel executionがmetadata dependenciesまたはLLM analysisでorderを決定する
**When**: 旧serial selection moduleが削除される
**Then**: parallel analyzerとorder-based dispatchの実装は変更されない
**And**: parallel selection結果は削除前と同等である
