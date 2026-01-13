# tui-key-hints Spec Delta

## ADDED Requirements
### Requirement: 未コミット change の操作ヒントを非表示にする
並列モードで未コミットの change が選択中の場合、Changes パネルのキーヒントは選択・承認に関する操作を表示してはならない（SHALL）。

#### Scenario: 未コミット change は選択ヒントを表示しない
- **GIVEN** TUI が並列モードで表示されている
- **AND** カーソルが未コミットの change にある
- **WHEN** Changes パネルを描画する
- **THEN** "Space: queue" と "@: approve" のキーヒントは表示されない

### Requirement: 未コミット change は操作不可として表示する
未コミットの change は Changes パネルで操作不可の状態として表示しなければならない（SHALL）。

#### Scenario: UNCOMMITED バッジを表示する
- **GIVEN** TUI が並列モードで表示されている
- **AND** change がコミットツリーに存在しない
- **WHEN** Changes パネルの行を描画する
- **THEN** 行はグレーアウトされる
- **AND** チェックボックスは表示されない
- **AND** `UNCOMMITED` バッジが表示される
