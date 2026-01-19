## ADDED Requirements
### Requirement: Web Dashboard Execution Controls
WebダッシュボードはTUIと同等の実行制御（開始/再開、停止、停止キャンセル、強制停止、リトライ）を提供しなければならない（SHALL）。

#### Scenario: 未実行状態の開始
- **GIVEN** Web UI が `app_mode = select` を受信している
- **WHEN** ユーザーが Run ボタンを押す
- **THEN** Web UI は制御APIに開始要求を送信する
- **AND** サーバーはTUIの開始処理と同じ経路で処理を開始する

#### Scenario: 停止状態の再開
- **GIVEN** Web UI が `app_mode = stopped` を受信している
- **WHEN** ユーザーが Run (Resume) ボタンを押す
- **THEN** Web UI は制御APIに再開要求を送信する
- **AND** サーバーはTUIの再開処理と同じ経路で実行マーク付き change をキューに戻して処理を再開する

#### Scenario: エラーモードの再実行
- **GIVEN** Web UI が `app_mode = error` を受信している
- **WHEN** ユーザーが Retry ボタンを押す
- **THEN** Web UI は制御APIに再実行要求を送信する
- **AND** サーバーはTUIのF5リトライと同じ経路でエラー change を再キューする

#### Scenario: 実行中の停止
- **GIVEN** Web UI が `app_mode = running` を受信している
- **WHEN** ユーザーが Stop ボタンを押す
- **THEN** サーバーはTUIの停止処理と同じ経路でグレースフル停止を開始する
- **AND** Web UI は `app_mode = stopping` を表示する

#### Scenario: 停止中の強制停止
- **GIVEN** Web UI が `app_mode = stopped` を受信している
- **WHEN** ユーザーが Force Stop を押す
- **THEN** Web UI は制御APIに強制停止要求を送信する
- **AND** サーバーは HTTP 409 を返す

#### Scenario: 停止キャンセル
- **GIVEN** Web UI が `app_mode = stopping` を受信している
- **WHEN** ユーザーが Cancel Stop を押す
- **THEN** サーバーはTUIの停止キャンセルと同じ経路で停止要求を取り消し、実行を継続する
- **AND** Web UI は `app_mode = running` を表示する

#### Scenario: 強制停止
- **GIVEN** Web UI が `app_mode = stopping` を受信している
- **WHEN** ユーザーが Force Stop を押す
- **THEN** サーバーはTUIの強制停止と同じ経路で現在のエージェントプロセスを終了し `Stopped` イベントを発行する
- **AND** Web UI は `app_mode = stopped` を表示する

### Requirement: Execution Control API
HTTPサーバーはWeb UIからの実行制御（開始/再開/停止/停止キャンセル/強制停止/リトライ）を受け付けるAPIを提供しなければならない（SHALL）。無効な状態遷移要求はHTTP 409で拒否し、状態を変更してはならない（MUST NOT）。

#### Scenario: 開始要求
- **WHEN** クライアントが `POST /api/control/start` を送信する
- **AND** サーバーが `app_mode` の開始可能状態である
- **THEN** サーバーは処理開始または再開を行う
- **AND** 成功時は HTTP 200 を返す

#### Scenario: 開始不可の状態
- **WHEN** `app_mode` が `running` または `stopping` である
- **AND** クライアントが `POST /api/control/start` を送信する
- **THEN** サーバーは HTTP 409 を返す
- **AND** 実行状態を変更しない

#### Scenario: 停止要求
- **WHEN** クライアントが `POST /api/control/stop` を送信する
- **AND** `app_mode` が `running` である
- **THEN** サーバーはグレースフル停止を開始する
- **AND** 成功時は HTTP 200 を返す

#### Scenario: 停止不可の状態
- **WHEN** `app_mode` が `select` または `stopped` である
- **AND** クライアントが `POST /api/control/stop` を送信する
- **THEN** サーバーは HTTP 409 を返す
- **AND** 実行状態を変更しない

#### Scenario: 停止キャンセル要求
- **WHEN** クライアントが `POST /api/control/cancel-stop` を送信する
- **AND** `app_mode` が `stopping` である
- **THEN** サーバーは停止要求を取り消し実行を継続する
- **AND** 成功時は HTTP 200 を返す

#### Scenario: 停止キャンセル不可の状態
- **WHEN** `app_mode` が `running` または `stopped` である
- **AND** クライアントが `POST /api/control/cancel-stop` を送信する
- **THEN** サーバーは HTTP 409 を返す
- **AND** 実行状態を変更しない

#### Scenario: 強制停止要求
- **WHEN** クライアントが `POST /api/control/force-stop` を送信する
- **AND** `app_mode` が `stopping` または `running` である
- **THEN** サーバーは実行中プロセスを終了し停止状態へ遷移する
- **AND** 成功時は HTTP 200 を返す

#### Scenario: 強制停止不可の状態
- **WHEN** `app_mode` が `select` または `stopped` である
- **AND** クライアントが `POST /api/control/force-stop` を送信する
- **THEN** サーバーは HTTP 409 を返す
- **AND** 実行状態を変更しない

#### Scenario: エラー再実行要求
- **WHEN** クライアントが `POST /api/control/retry` を送信する
- **AND** `app_mode` が `error` である
- **THEN** サーバーはエラー change を再キューして処理を再開する
- **AND** 成功時は HTTP 200 を返す

#### Scenario: リトライ不可の状態
- **WHEN** `app_mode` が `select` または `running` である
- **AND** クライアントが `POST /api/control/retry` を送信する
- **THEN** サーバーは HTTP 409 を返す
- **AND** 実行状態を変更しない

### Requirement: Web App Mode Vocabulary
WebSocketの `app_mode` はTUIと同じ語彙で通知されなければならない（SHALL）。`select/running/stopping/stopped/error` を最低限含まなければならない（MUST）。

#### Scenario: 追加されたapp_modeを配信する
- **WHEN** 実行状態が停止中または停止処理中になる
- **THEN** `app_mode` は `stopped` または `stopping` を通知する
- **AND** `select/running/error` と同一の語彙で運用される

#### Scenario: エラーモードの通知
- **WHEN** 実行中にエラーが発生する
- **THEN** `app_mode` は `error` を通知する
