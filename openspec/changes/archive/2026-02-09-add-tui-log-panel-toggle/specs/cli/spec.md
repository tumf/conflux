## ADDED Requirements
### Requirement: Log Panel Visibility Toggle
TUI は Changes ビューで `l` キーによりログパネルの表示/非表示を切り替えられるようにしなければならない（SHALL）。
ログパネルの既定状態は表示（有効）でなければならない（SHALL）。

#### Scenario: Toggle off hides log panel while keeping logs
- **GIVEN** ログパネルが有効である
- **AND** ログが存在する
- **WHEN** ユーザーが `l` キーを押す
- **THEN** ログパネルは非表示になる
- **AND** 新しいログは引き続きログバッファに追加される

#### Scenario: Toggle on restores log panel
- **GIVEN** ログパネルが無効である
- **WHEN** ユーザーが `l` キーを押す
- **THEN** ログが存在する場合、ログパネルが表示される

## MODIFIED Requirements
### Requirement: TUI Layout Structure

The TUI SHALL display appropriate layout for Stopping and Stopped modes in addition to existing modes.

#### Scenario: Stopping mode layout

- **WHEN** TUI is in Stopping mode
- **THEN** header displays "Stopping..." status in yellow
- **AND** current processing panel shows "Completing..."
- **AND** ログパネルが有効な場合は停止メッセージを含むログパネルが表示される
- **AND** ログパネルが無効な場合でも停止メッセージはログに記録される

#### Scenario: Stopped mode layout

- **WHEN** TUI is in Stopped mode
- **THEN** header displays "Stopped" status in gray
- **AND** status panel shows summary of completed/queued changes
- **AND** footer shows available actions (F5: resume, q: quit)
