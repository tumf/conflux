## ADDED Requirements

### Requirement: 中断されたApplyはworkspace-local progressを保存する

active managed-worktree Applyがcancelまたはabsolute runtime-limit expiryで終了する場合、Confluxは最初にowned process groupのquiescenceを証明し、その後にdirtyなstaged・unstaged・untracked workspace progressを既存Conflux-owned WIP snapshot pathで保存しなければならない（MUST）。interruption outcomeはsame-run automatic redispatchを行わずactive runを停止しなければならない（MUST）。cleanupまたはsnapshot creationが失敗した場合、Confluxはworkspace contentsを保持してactionable diagnosticsを返し、successful preservationを報告またはAcceptanceをdispatchしてはならない（MUST NOT）。

#### Scenario: Operator cancellationはdirty Apply progressを保存する

- **GIVEN** managed Apply commandがstaged、unstaged、untracked fileを変更している
- **WHEN** active Applyがcancelされる
- **THEN** Confluxはcommand admissionを閉じowned process groupを終了する
- **AND** repository mutation前にprocess-group quiescenceを証明する
- **AND** dirty workspace progressを含むWIP snapshotを作成する
- **AND** Applyを同じrunで自動redispatchせずactive runを停止する

#### Scenario: Runtime-limit expiryはdirty Apply progressを保存する

- **GIVEN** managed Apply commandがworkspaceを変更している
- **AND** commandがabsolute runtime limitへ到達する
- **WHEN** process-group cleanupがquiescenceを確認する
- **THEN** Confluxは既存workspace managerでWIP snapshotを1つ作成する
- **AND** runtime-limit outcomeはactive run内でnon-retryableである
- **AND** Acceptanceをdispatchしない

#### Scenario: Clean interruptionはempty WIP snapshotを作らない

- **GIVEN** active managed Applyがstaged、unstaged、untracked workspace stateを変更していない
- **AND** commandがcancelまたはabsolute runtime limitへ到達する
- **WHEN** process-group cleanupがquiescenceを確認する
- **THEN** ConfluxはWIP snapshot pathを呼ばない
- **AND** empty WIP commitを作成しない
- **AND** same-run automatic redispatchを行わずactive runを停止する

#### Scenario: Restartは保存されたworkspaceからcontinuationを導出する

- **GIVEN** interrupted ApplyがWIP snapshotを作成している
- **WHEN** external stateとlogを削除したfresh processでConfluxを起動する
- **THEN** 次actionをworkspace file、Git history、base comparisonから導出する
- **AND** changeを未開始ではなく既存Apply workとしてresumeする

#### Scenario: Snapshot failureはrecoverable fileを保持する

- **GIVEN** interrupted Applyがdirtyでprocess groupがquiescentである
- **WHEN** WIP snapshot creationが失敗する
- **THEN** Confluxはworkspaceとindex contentsをrecovery用に保持する
- **AND** snapshot diagnosticsを伴うnon-zeroを返す
- **AND** successful interruption recoveryを報告しない
