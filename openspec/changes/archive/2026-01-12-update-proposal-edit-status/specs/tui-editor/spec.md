## ADDED Requirements
### Requirement: Proposal編集時のオーケストレーションステータス維持
TUIでproposal編集を開始・終了しても、オーケストレーションステータスは変更してはならない（MUST）。

#### Scenario: Proposal編集開始
- **GIVEN** TUIが選択モードであり、現在のオーケストレーションステータスが表示されている
- **WHEN** ユーザーが `e` キーでproposal編集を開始する
- **THEN** オーケストレーションステータスは編集開始前の値を維持する
- **AND** ヘッダのステータス表示は変更されない

#### Scenario: Proposal編集終了
- **GIVEN** proposal編集のためにエディタが起動している
- **WHEN** ユーザーがエディタを終了しTUIが復帰する
- **THEN** オーケストレーションステータスは編集開始前の値を維持する
