## ADDED Requirements

### Requirement: git sync API の責務分割安全性

Git sync API の内部リファクタリングは、resolve command 実行、sync planning、pull/push/sync route contract を個別に検証可能な形で保持しなければならない。

#### Scenario: git sync API の代表分岐が維持される

- **GIVEN** local/remote SHA、resolve command 設定、git repository fixture が用意されている
- **WHEN** pull、push、sync、resolve command の代表経路を実行する
- **THEN** skip、non-fast-forward、auto-resolve、success response の判定はリファクタ前と同等である
- **AND** API の route、status code、response body は変更されない
