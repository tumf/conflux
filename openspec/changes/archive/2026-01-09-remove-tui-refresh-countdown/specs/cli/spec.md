# cli Spec Delta

## REMOVED Requirements

### Requirement: 自動更新インジケーター (Scenario removal)

The countdown indicator scenario SHALL be removed from the auto-refresh requirement.

#### Scenario: 自動更新インジケーター (REMOVED)
- ~~**WHEN** TUIが表示されている~~
- ~~**THEN** ヘッダーに自動更新間隔とインジケーター（`Auto-refresh: 5s ↻`）が表示される~~

**Rationale**: The countdown display adds visual noise without providing meaningful value. The auto-refresh functionality continues to work; only the visual indicator is removed.

## MODIFIED Requirements

### Requirement: TUIレイアウト構成

TUI SHALL display appropriate layouts for selection and running modes without auto-refresh countdown indicator.

#### Scenario: 選択モードのレイアウト (MODIFIED)
- **WHEN** TUIが選択モードである
- **THEN** ヘッダー（タイトル、モード表示）が上部に表示される
- **AND** 操作ヘルプ（↑↓: move, Space: toggle, F5: run, q: quit）が表示される
- **AND** チェックボックス付き変更リストが中央に表示される
- **AND** 選択件数・新規件数がフッターに表示される
- **AND** アプリケーション状態に応じたガイダンスメッセージがフッターに表示される

**Change**: Removed "自動更新インジケーター" from header description.

#### Scenario: 実行モードのレイアウト (MODIFIED)
- **WHEN** TUIが実行モードである
- **THEN** ヘッダー（タイトル、Running/Completedステータス）が上部に表示される
- **AND** キュー状態付き変更リストが表示される
- **AND** 現在処理パネル（変更ID、ステータス）が表示される
- **AND** ログパネルが下部に表示される

**Change**: Removed "自動更新インジケーター" from header description.
