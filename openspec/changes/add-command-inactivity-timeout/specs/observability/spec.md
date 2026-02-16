## ADDED Requirements
### Requirement: 無出力タイムアウトの警告ログ

オーケストレーターは無出力タイムアウトを検知した場合、警告ログを出力しなければならない (MUST)。

警告ログには以下を含めなければならない (MUST)：
- どの操作で発生したか（apply/archive/resolve/analyze/acceptance）
- 対象の change_id（該当する場合）
- 無出力継続時間と設定タイムアウト値

#### Scenario: 無出力タイムアウトの警告ログ
- **GIVEN** apply 実行中に無出力タイムアウトが発生する
- **WHEN** タイムアウト検知が行われる
- **THEN** warning ログが出力される
- **AND** ログに操作種別と change_id が含まれる
