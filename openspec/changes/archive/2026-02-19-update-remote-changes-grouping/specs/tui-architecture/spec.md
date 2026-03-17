## MODIFIED Requirements

### Requirement: プロジェクト単位のグルーピング表示

TUI は `--server` 指定時の change 一覧をプロジェクト単位でグルーピングして表示しなければならない（SHALL）。各プロジェクトは見出し行として表示し、見出し行は選択や操作の対象にしてはならない（MUST NOT）。各 change 行はプロジェクト名を重複表示せず、change_id のみを表示しなければならない（SHALL）。カーソル移動と選択/実行の操作は change 行のみを対象にしなければならない（SHALL）。

#### Scenario: プロジェクトごとに表示が区切られる
- **GIVEN** サーバに 2 つのプロジェクトが登録されている
- **WHEN** TUI が `--server` 指定で change 一覧を表示する
- **THEN** change 一覧はプロジェクト見出しで区切られて表示される
- **AND** 各 change 行には change_id のみが表示される

#### Scenario: 見出し行は選択対象にならない
- **GIVEN** サーバに 2 つのプロジェクトが登録されている
- **WHEN** ユーザーが ↑↓ でカーソル移動し、Space で選択を切り替える
- **THEN** カーソルは change 行にのみ移動する
- **AND** 見出し行は選択や操作の対象にならない
