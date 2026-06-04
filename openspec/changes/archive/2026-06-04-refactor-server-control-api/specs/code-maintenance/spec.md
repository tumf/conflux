## ADDED Requirements

### Requirement: server control API の責務分割安全性

server control API の内部リファクタリングは、既存の route contract と状態更新の副作用を保持するために、代表的な control 操作を characterization test で固定しなければならない。

#### Scenario: control API route contract が維持される

- **GIVEN** 既存の project と change selection 状態がある
- **WHEN** selection、global run、stop/dequeue、stats/logs の代表 control API を呼び出す
- **THEN** HTTP status、response body、selection state、WebSocket update の発火条件はリファクタ前と同等である
- **AND** API path、method、認証要件は変更されない
