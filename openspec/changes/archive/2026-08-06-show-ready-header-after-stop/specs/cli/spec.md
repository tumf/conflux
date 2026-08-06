## MODIFIED Requirements

### Requirement: Running Mode Dashboard

TUI は Running モードでダッシュボード形式の UI を表示しなければならない（SHALL）。
正常完了時は Ready 表示に戻り、停止要求がない限り Stopped へ遷移してはならない。

TUI が shared reducer の display snapshot を `AppState` に同期する場合、Running mode の in-flight 状態を表す execution lifecycle events を reducer に反映してから display snapshot を適用しなければならない（MUST）。これにより、`ChangesRefreshed` 後も active display status と header count が stale reducer snapshot によって失われてはならない（MUST NOT）。

ヘッダーステータスは現在のオーケストレーション活動を表示し、内部の停止後再開制御状態を新しい実行中ステータスとして公開してはならない（MUST NOT）。`AppExecutionMode::Select` と `AppExecutionMode::Stopped` は `Ready`、`Running` は `Running` または `Running <count>`、`Stopping` は `Stopping` を表示する。`Error` は既存どおりステータスラベルを表示しない。内部 Stopped mode は resume routing と controls のために維持し、Header projectionによって変更してはならない（MUST NOT）。

#### Scenario: Display on processing completion
- **WHEN** すべての queued change が処理完了する
- **THEN** ヘッダーステータスが "Ready" に切り替わる
- **AND** TUI は Select（Ready）モードに戻る
- **AND** ステータスパネルは進捗と経過時間のみを表示する
- **AND** `Ctrl+C` で終了できるよう表示を維持する

#### Scenario: Running mode header shows processing count
- **GIVEN** TUI が Running モードである
- **WHEN** 1 件以上の change が in-flight 状態（Applying/Accepting/Archiving/Resolving）である
- **THEN** ヘッダーは "Running <count>" を表示し、<count> は in-flight change の件数になる
- **AND** queued の change は <count> に含めない

#### Scenario: Reducer display sync preserves active header count
- **GIVEN** TUI が Running モードである
- **AND** shared reducer display snapshot が `AppState` の表示状態に同期される
- **WHEN** `ApplyStarted`, `AcceptanceStarted`, `ArchiveStarted`, or `ResolveStarted` が発生し、その後 `ChangesRefreshed` が発生する
- **THEN** 当該 change の表示状態は in-flight 状態として保持される
- **AND** ヘッダーは active change 数を `Running <count>` として表示し続ける
- **AND** queued のみの change は <count> に含めない

#### Scenario: Stopped mode header projects Ready
- **GIVEN** TUI の内部 execution mode が Stopped である
- **WHEN** ヘッダーが描画される
- **THEN** ヘッダーは cyan の `Ready` ステータスを表示する
- **AND** ヘッダーは `Stopped` ステータスを表示しない
- **AND** 内部 execution mode は Stopped のまま維持される

#### Scenario: Error mode header remains unlabeled
- **GIVEN** TUI が Error モードである
- **WHEN** ヘッダーが描画される
- **THEN** ヘッダーはステータスラベルを表示しない

### Requirement: TUI Layout Structure

The TUI SHALL display appropriate layout for Stopping and Stopped modes in addition to existing modes. Stopped mode SHALL use the Ready header projection while retaining stopped-mode resume controls.

#### Scenario: Stopping mode layout

- **WHEN** TUI is in Stopping mode
- **THEN** header displays "Stopping..." status in yellow
- **AND** current processing panel shows "Completing..."
- **AND** ログパネルが有効な場合は停止メッセージを含むログパネルが表示される
- **AND** ログパネルが無効な場合でも停止メッセージはログに記録される

#### Scenario: Stopped mode layout

- **WHEN** TUI is in Stopped mode
- **THEN** header displays "Ready" status in cyan
- **AND** status panel shows summary of completed/queued changes
- **AND** footer shows available actions (F5: resume, q: quit)
- **AND** rendering does not change the internal Stopped mode

### Requirement: TUI Stopped Mode

The TUI SHALL provide an internal Stopped mode that manages change state by holding queued status only during execution. When transitioning to Stopped, queue_status SHALL be reset to NotQueued while preserving execution marks ([x]). Space operations in Stopped mode SHALL only add/remove execution marks while maintaining queue_status as NotQueued. When resuming with F5, execution-marked changes SHALL be restored to queued and processing SHALL resume. Task progress updates in Stopped mode SHALL NOT trigger queuing. The header SHALL project this inactive resumable mode as `Ready`; mode-specific controls SHALL continue to identify F5 as `resume`.

#### Scenario: Stopped mode display
- **WHEN** TUI is in Stopped mode
- **THEN** header status displays "Ready" in cyan color
- **AND** status controls display the configured start key as `resume`
- **AND** the change list remains visible with current statuses
- **AND** execution-marked changes show "[x]" while their queue_status remains not queued
- **AND** the internal execution mode remains Stopped

#### Scenario: Queue management in Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on an execution-marked change
- **THEN** the execution mark is removed and queue_status remains not queued

#### Scenario: Queue addition in Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** user presses Space on a not-marked change
- **THEN** the execution mark is added and queue_status remains not queued

#### Scenario: Task completion in Stopped mode does not auto-queue
- **WHEN** TUI is in Stopped mode
- **AND** a change's tasks are updated (e.g., all tasks marked complete)
- **THEN** the change queue_status SHALL remain not queued
- **AND** the change SHALL NOT be automatically added to the queue

#### Scenario: Resume processing from Stopped mode
- **WHEN** TUI is in Stopped mode
- **AND** one or more changes are execution-marked
- **AND** user presses F5
- **THEN** the TUI transitions to Running mode
- **AND** processing resumes after converting execution-marked changes to queued
- **AND** log displays "Resuming processing..."

#### Scenario: Resume with empty queue shows warning
- **WHEN** TUI is in Stopped mode
- **AND** no changes are execution-marked
- **AND** user presses F5
- **THEN** a warning message is displayed
- **AND** the TUI remains in Stopped mode
