## ADDED Requirements

### Requirement: 受け入れ判定ロジックの出力一貫性
システムは受け入れ判定（PASS/FAIL/CONTINUE/BLOCKED）と失敗所見（findings）の生成において、同一のコマンド出力データを情報源として扱わなければならない。

#### Scenario: FAIL時に判定結果と所見の由来が一致する
- **GIVEN** 受け入れコマンドが `ACCEPTANCE: FAIL` と複数の所見を出力する
- **WHEN** オーケストレーションが受け入れ結果を解析する
- **THEN** 判定は FAIL となる
- **AND** 記録される `findings` は同一出力から抽出された内容と一致する

#### Scenario: PASS時に既存の判定結果が維持される
- **GIVEN** 受け入れコマンドが `ACCEPTANCE: PASS` を出力する
- **WHEN** 受け入れ結果を解析する
- **THEN** 判定は PASS となる
- **AND** CLIの公開挙動（引数、終了コード、出力形式）は変更されない
