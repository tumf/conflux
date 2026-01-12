# Circuit Breaker Capability

## ADDED Requirements

### Requirement: 出力減少検出

Orchestratorはエージェントの出力量が異常に減少した場合に検出し、停滞状態を早期に識別しなければならない（SHALL）。

#### Scenario: 出力が70%以上減少した場合に警告とスキップを行う
- **GIVEN** 前回のapply実行で1000バイトの出力があった
- **AND** 今回のapply実行で200バイトの出力があった
- **WHEN** 出力減少率を計算する
- **THEN** 80%減少と判定される
- **AND** warningログが出力される
- **AND** changeがスキップされる

#### Scenario: 正常な出力減少では検出されない
- **GIVEN** 前回のapply実行で1000バイトの出力があった
- **AND** 今回のapply実行で500バイトの出力があった
- **WHEN** 出力減少率を計算する
- **THEN** 50%減少でしきい値未満となる
- **AND** 検出されず通常処理が継続される

#### Scenario: 初回実行では検出されない
- **GIVEN** あるchangeが初めてapplyされる
- **AND** 出力履歴が存在しない
- **WHEN** 出力減少検出を実行する
- **THEN** 比較対象がないため検出されない
- **AND** 今回の出力が履歴に記録される

#### Scenario: 設定で減少率しきい値を変更できる
- **GIVEN** config内で`output_decline_detector.threshold_percent = 50`が設定されている
- **WHEN** 出力が60%減少する
- **THEN** しきい値を超えたため検出される
- **AND** changeがスキップされる
