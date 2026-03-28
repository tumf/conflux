## ADDED Requirements

### Requirement: サーバダッシュボード UI の提供

`cflx server` は `/dashboard` パスでブラウザ向けの Web ダッシュボード UI を提供しなければならない（SHALL）。ダッシュボードは React + shadcn/ui で構築し、ビルド済み静的ファイルをバイナリに埋め込んで配信する。

#### Scenario: ダッシュボードにアクセスできる

- **GIVEN** `cflx server` が起動している
- **WHEN** ブラウザで `http://<host>:<port>/dashboard` にアクセスする
- **THEN** サーバは HTTP 200 で HTML ページを返す
- **AND** ダッシュボード UI が表示される

#### Scenario: ダッシュボードが WebSocket でリアルタイム状態を受信する

- **GIVEN** ダッシュボードがブラウザで表示されている
- **WHEN** WebSocket `/api/v1/ws` に接続される
- **THEN** `FullState` メッセージを受信して全プロジェクトの状態を更新する
- **AND** `Log` メッセージを受信してログパネルにリアルタイム表示する

### Requirement: ダッシュボードからのプロジェクト操作

ダッシュボードは各プロジェクトに対して Run / Stop / Git Sync / Delete の操作を提供しなければならない（SHALL）。

#### Scenario: ダッシュボードから実行を開始できる

- **GIVEN** ダッシュボードにプロジェクトが表示されている
- **WHEN** ユーザーが Run ボタンを押す
- **THEN** `POST /api/v1/projects/{id}/control/run` が呼び出される
- **AND** 結果がトースト通知で表示される

#### Scenario: ダッシュボードからプロジェクトを削除できる

- **GIVEN** ダッシュボードにプロジェクトが表示されている
- **WHEN** ユーザーが Delete ボタンを押す
- **THEN** 確認ダイアログが表示される
- **AND** 確認後に `DELETE /api/v1/projects/{id}` が呼び出される

### Requirement: ダッシュボードの WebSocket 再接続

ダッシュボードは WebSocket 切断時に exponential backoff で自動再接続しなければならない（SHALL）。

#### Scenario: WebSocket 切断後に再接続する

- **GIVEN** ダッシュボードが WebSocket に接続している
- **WHEN** WebSocket 接続が切断される
- **THEN** ダッシュボードは exponential backoff（1s → 2s → 4s → max 30s）で再接続を試みる
- **AND** 接続状態インジケータが Reconnecting に変わる
- **AND** 再接続成功後に Connected に戻る
