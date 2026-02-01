## ADDED Requirements
### Requirement: Changes一覧ログプレビューの相対時間表記
TUIのChanges一覧に表示されるログプレビューは、相対時間を括弧で囲んだ形式で表示しなければならない（SHALL）。

#### Scenario: 相対時間を括弧で囲む
- **GIVEN** Changes一覧にログプレビューが表示される
- **WHEN** TUIがChanges一覧を描画する
- **THEN** ログプレビューの相対時間は括弧付き形式（例: `(2m ago)`）で表示される

### Requirement: カーソル行のログプレビュー視認性
TUIのChanges一覧でカーソル行が選択されている場合、ログプレビューの文字色は非選択行より明るく表示しなければならない（SHALL）。

#### Scenario: カーソル行でログプレビューが判読できる
- **GIVEN** Changes一覧のカーソル行が選択されている
- **AND** 該当行にログプレビューが表示されている
- **WHEN** TUIがChanges一覧を描画する
- **THEN** ログプレビューは選択背景上でも判読できる明るい文字色で表示される
