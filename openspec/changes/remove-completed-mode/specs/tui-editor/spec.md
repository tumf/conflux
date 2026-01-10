# tui-editor Specification Delta

## REMOVED Requirements

### Requirement: 完了モードではエディタ起動不可

This requirement SHALL be removed because `Completed` mode no longer exists. After all processing completes, TUI returns to Select mode where editor launch is available.

#### Scenario: 完了モードではエディタ起動不可

- **GIVEN** TUIがCompletedモードである
- **WHEN** ユーザーが `e` キーを押す
- **THEN** エディタは起動しない

**Rationale**: Completed mode is removed; TUI returns to Select mode after completion.
