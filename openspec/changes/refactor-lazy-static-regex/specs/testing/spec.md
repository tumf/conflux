## MODIFIED Requirements

### Requirement: Spec Test Annotations Parsing

`spec_test_annotations` モジュールは仕様ファイルからシナリオを正しくパースしなければならない (SHALL)。

LazyLock 移行後も、パース結果が従来と同一であることをテストスイートで担保しなければならない (MUST)。

#### Scenario: LazyLock 移行後もパース結果が同一

- **GIVEN** `src/spec_test_annotations.rs` の正規表現が `LazyLock<Regex>` に移行済みである
- **WHEN** 既存の spec テストアノテーションパーサーテストを実行する
- **THEN** すべてのテストが移行前と同一の結果を返す
