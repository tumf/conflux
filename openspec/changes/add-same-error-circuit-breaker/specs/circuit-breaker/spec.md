# Circuit Breaker Capability

## ADDED Requirements

### Requirement: Same error detection

The Orchestrator MUST detect when the same error occurs repeatedly and prevent stack loops.

#### Scenario: 5回連続で同じエラーが発生した場合、changeをスキップ

**Given** あるchangeが5回連続でapplyされている  
**And** 各apply実行で同じエラーメッセージが発生している  
**When** orchestratorが6回目のapplyを試みようとする  
**Then** 同一エラー検出を行いerrorログを出力する  
**And** そのchangeをスキップして次へ移行する

#### Scenario: エラーメッセージの正規化により同一性を判定

**Given** 1回目のエラーが"File not found: /path/to/file1"  
**And** 2回目のエラーが"File not found: /path/to/file2"  
**When** エラーメッセージを正規化して比較する  
**Then** パス部分を除外して"File not found"パターンとして認識される  
**And** 同一エラーとしてカウントされる

#### Scenario: JSONフィールド名が誤検知されない

**Given** エージェント出力に`"is_error": false`というJSONフィールドが含まれる  
**When** エラー検出処理を実行する  
**Then** JSONフィールド名は除外される  
**And** 誤ってエラーとして検出されない

#### Scenario: 異なるエラーが混在する場合は検出されない

**Given** 1回目が"File not found"エラー  
**And** 2回目が"Permission denied"エラー  
**And** 3回目が"File not found"エラー  
**When** 同一エラー検出を実行する  
**Then** 連続していないため検出されない  
**And** 通常通り処理が継続される

#### Scenario: 設定でエラー検出しきい値を変更できる

**Given** config内で`error_circuit_breaker.threshold = 3`が設定されている  
**When** 3回連続で同じエラーが発生する  
**Then** 3回目で同一エラー検出が行われる  
**And** changeがスキップされる
