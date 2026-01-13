## MODIFIED Requirements

### Requirement: TUI debug log deduplication

TUIモードは、状態が変化していない場合に重複したDEBUGログを出力してはならない (MUST)。

#### Scenario: 状態が変化しない場合

- **WHEN** 同一のchange状態が連続して観測される
- **THEN** 重複したDEBUGログは出力されない
