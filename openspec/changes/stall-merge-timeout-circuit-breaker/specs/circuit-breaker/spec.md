## ADDED Requirements
### Requirement: merge停滞のサーキットブレーカ

オーケストレーターは、ベースブランチに対する merge 進捗が一定時間発生しない場合に stall と判定し、実行中を含む全処理を即時停止しなければならない（MUST）。

#### Scenario: 30分間 merge 進捗が無い場合に即時停止
- **GIVEN** オーケストレーションが実行中である
- **AND** ベースブランチに `Merge change: <change_id>` の merge コミットが 30 分以上追加されていない
- **WHEN** 監視チェックが実行される
- **THEN** stall と判定される
- **AND** 実行中を含む全処理が即時停止する
- **AND** 停止理由として merge 停滞がログに記録される

#### Scenario: 監視期間内に merge 進捗が発生した場合は継続
- **GIVEN** オーケストレーションが実行中である
- **AND** ベースブランチに `Merge change: <change_id>` の merge コミットが監視期間内に追加されている
- **WHEN** 監視チェックが実行される
- **THEN** stall 判定は行われず処理が継続する

#### Scenario: serial と parallel の両モードで適用する
- **GIVEN** オーケストレーターが serial もしくは parallel モードで実行中である
- **WHEN** merge 停滞が検出される
- **THEN** どちらのモードでも同様に即時停止する

## MODIFIED Requirements
### Requirement: 同一エラー検出

Orchestratorは同一エラーが連続して発生した場合に検出し、無限ループを防止しなければならない（SHALL）。

#### Scenario: 5回連続で同じエラーが発生した場合、changeをスキップ
- **GIVEN** あるchangeが5回連続でapplyされている
- **AND** 各apply実行で同じエラーメッセージが発生している
- **WHEN** orchestratorが6回目のapplyを試みようとする
- **THEN** 同一エラー検出を行いerrorログを出力する
- **AND** そのchangeをスキップして次へ移行する

#### Scenario: エラーメッセージの正規化により同一性を判定
- **GIVEN** 1回目のエラーが"File not found: /path/to/file1"である
- **AND** 2回目のエラーが"File not found: /path/to/file2"である
- **WHEN** エラーメッセージを正規化して比較する
- **THEN** パス部分を除外して"File not found"パターンとして認識される
- **AND** 同一エラーとしてカウントされる

#### Scenario: JSONフィールド名が誤検知されない
- **GIVEN** エージェント出力に`"is_error": false`というJSONフィールドが含まれる
- **WHEN** エラー検出処理を実行する
- **THEN** JSONフィールド名は除外される
- **AND** 誤ってエラーとして検出されない

#### Scenario: 異なるエラーが混在する場合は検出されない
- **GIVEN** 1回目が"File not found"エラーである
- **AND** 2回目が"Permission denied"エラーである
- **AND** 3回目が"File not found"エラーである
- **WHEN** 同一エラー検出を実行する
- **THEN** 連続していないため検出されない
- **AND** 通常通り処理が継続される

#### Scenario: 設定でエラー検出しきい値を変更できる
- **GIVEN** config内で`error_circuit_breaker.threshold = 3`が設定されている
- **WHEN** 3回連続で同じエラーが発生する
- **THEN** 3回目で同一エラー検出が行われる
- **AND** changeがスキップされる
