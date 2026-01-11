# Circuit Breaker Capability

## ADDED Requirements

### Requirement: Output decline detection

The Orchestrator MUST detect when the agent's output volume decreases abnormally and identify "nothing left to do" states early.

#### Scenario: 出力が70%以上減少した場合に警告

**Given** 前回のapply実行で1000バイトの出力があった  
**And** 今回のapply実行で200バイトの出力があった  
**When** 出力減少率を計算する  
**Then** 80%減少と判定される  
**And** warningログが出力される  
**And** changeがスキップされる

#### Scenario: 正常な出力減少では検出されない

**Given** 前回のapply実行で1000バイトの出力があった  
**And** 今回のapply実行で500バイトの出力があった  
**When** 出力減少率を計算する  
**Then** 50%減少でしきい値未満  
**And** 検出されず通常処理が継続される

#### Scenario: 初回実行では検出されない

**Given** あるchangeが初めてapplyされる  
**And** 出力履歴が存在しない  
**When** 出力減少検出を実行する  
**Then** 比較対象がないため検出されない  
**And** 今回の出力が履歴に記録される

#### Scenario: 設定で減少率しきい値を変更できる

**Given** config内で`output_decline_detector.threshold_percent = 50`が設定されている  
**When** 出力が60%減少する  
**Then** しきい値を超えたため検出される  
**And** changeがスキップされる
