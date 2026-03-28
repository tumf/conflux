## ADDED Requirements

### Requirement: ダッシュボードUIからのプロジェクト追加

Webダッシュボードは「+ Add Project」ボタンを提供し、ユーザーが `remote_url` と `branch` を入力してプロジェクトを追加できるモーダルダイアログを表示しなければならない（SHALL）。

ダイアログ送信時にダッシュボードは `POST /api/v1/projects` を呼び出し、成功・失敗をトースト通知で伝えなければならない（MUST）。

#### Scenario: 正常なプロジェクト追加

- **WHEN** ユーザーがダッシュボードの「+ Add Project」ボタンをクリックする
- **THEN** `remote_url` と `branch` の入力フォームを含むダイアログが表示される

#### Scenario: フォーム送信で API を呼び出す

- **GIVEN** ユーザーが `remote_url` と `branch` を入力した
- **WHEN** ユーザーが Submit ボタンをクリックする
- **THEN** ダッシュボードは `POST /api/v1/projects` に `{ remote_url, branch }` を送信する
- **AND** 成功時はトースト通知「Project added」を表示する

#### Scenario: 追加失敗時のエラー通知

- **GIVEN** サーバーがエラー（422 や 409 など）を返した
- **WHEN** ユーザーが Submit ボタンをクリックする
- **THEN** ダッシュボードはエラー内容を含むトースト通知を表示する
- **AND** ダイアログは閉じない
