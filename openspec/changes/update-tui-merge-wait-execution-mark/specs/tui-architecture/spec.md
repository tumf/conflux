## MODIFIED Requirements
### Requirement: Queue State Synchronization

システムは、UI上のキュー状態とDynamicQueueの状態を常に同期させなければならない（SHALL）。

`ResolveWait`はresolve待機中の状態であり、Space/@によるキュー操作でDynamicQueueを変更してはならない（MUST NOT）。`MergeWait`も同様に、キュー操作の対象としてはならない（MUST NOT）。

ただし、`ResolveWait`/`MergeWait`の行ではSpace操作による実行マーク（`selected`）のトグルを許可しなければならない（SHALL）。この操作は`queue_status`およびDynamicQueueを変更してはならない（MUST NOT）。

TUIは`ResolveWait`を`resolve wait`として表示し、操作対象外（キュー操作不可）であることが明確でなければならない（MUST）。

#### Scenario: Unapproveによるキューからの削除
- **WHEN** ユーザーが@キーでqueuedのchangeをunapprove
- **THEN** `QueueStatus::NotQueued` に変更され、DynamicQueueからも削除される

#### Scenario: Spaceキーによるキューからの削除
- **WHEN** ユーザーがRunningモード中にSpaceキーで [x] のchangeをdequeue
- **THEN** `QueueStatus::NotQueued` に変更され、DynamicQueueからも削除される

#### Scenario: 削除操作のログ記録
- **WHEN** DynamicQueueからchangeが削除される
- **THEN** ログに削除操作が記録される

#### Scenario: ResolveWait中はキュー状態を変更できない
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `ResolveWait`
- **WHEN** the user presses Space or `@`
- **THEN** the change status SHALL remain `ResolveWait`
- **AND** DynamicQueue SHALL NOT be modified for the change
- **AND** Space操作は実行マークのみをトグルする

#### Scenario: MergeWait中はキュー状態を変更できない
- **GIVEN** the TUI is in running mode
- **AND** the cursor is on a change in `MergeWait`
- **WHEN** the user presses Space
- **THEN** the change status SHALL remain `MergeWait`
- **AND** DynamicQueue SHALL NOT be modified for the change
- **AND** Space操作は実行マークのみをトグルする
