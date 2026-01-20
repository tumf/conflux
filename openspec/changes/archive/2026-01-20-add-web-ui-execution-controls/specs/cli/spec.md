## ADDED Requirements
### Requirement: Web Execution Control Availability
Web UIからの実行制御は、`--web` でHTTPサーバーが起動している場合にのみ有効でなければならない（SHALL）。TUIとRunモードのいずれでも同じ制御経路を提供しなければならない（MUST）。RunモードではTUIと同等のリトライ/停止挙動を提供しなければならない（SHALL）。

#### Scenario: TUIモードでのWeb制御
- **GIVEN** `cflx tui --web` で起動している
- **WHEN** Web UI が制御APIへ開始/停止要求を送る
- **THEN** TUIの実行状態が同等に変化する

#### Scenario: RunモードでのWeb制御
- **GIVEN** `cflx run --web` で起動している
- **WHEN** Web UI が制御APIへ開始/停止要求を送る
- **THEN** オーケストレーターの実行状態が同等に変化する

#### Scenario: Runモードでのリトライ制御
- **GIVEN** `cflx run --web` で実行中にエラーが発生している
- **WHEN** Web UI が制御APIへ retry 要求を送る
- **THEN** オーケストレーターは同一のエラー change を再実行する
