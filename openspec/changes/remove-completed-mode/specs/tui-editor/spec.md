# tui-editor Specification Delta

## REMOVED Requirements

### Requirement: 完了モードではエディタ起動不可 (REMOVED)

This requirement is removed because `Completed` mode no longer exists.

#### Scenario: 完了モードではエディタ起動不可 (REMOVED)

- **GIVEN** TUIがCompletedモードである
- **WHEN** ユーザーが `e` キーを押す
- **THEN** エディタは起動しない

**Rationale**: After all processing completes, TUI returns to Select mode where editor launch is available.

## ADDED Requirements

### Requirement: 処理完了後のSelectモード復帰

全ての処理が完了した後、TUIはSelectモードに復帰しなければならない（SHALL）。

#### Scenario: 全処理完了後にSelectモードへ遷移

- **GIVEN** TUIが実行モード（Running）である
- **AND** キューに入った全ての変更の処理が完了した
- **WHEN** 最後の変更の処理が完了する
- **THEN** TUIはSelectモードに遷移する
- **AND** 完了メッセージがログに表示される
- **AND** ユーザーは即座に次の変更を選択・キュー追加できる
- **AND** `e` キーでエディタを起動できる
