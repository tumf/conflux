## MODIFIED Requirements
### Requirement: Apply Context History

オーケストレーターは、逐次/並列のどちらの apply でも共通ループで同一の履歴注入ロジックを使用し、各 apply 試行の最終サマリーメッセージを記録して同一 change の次回 apply プロンプトに含めなければならない（MUST）。

#### Scenario: parallel の2回目 apply に履歴が含まれる
- **GIVEN** parallel mode で change が apply 実行中である
- **AND** 1回目の apply がエージェントのサマリーを返している
- **WHEN** 2回目の apply が実行される
- **THEN** プロンプトは base apply_prompt を含む
- **AND** プロンプトは `<last_apply attempt="1">` ブロックを含む
- **AND** ブロックには 1回目のサマリーが含まれる

#### Scenario: serial の2回目 apply に履歴が含まれる
- **GIVEN** 逐次モードで change が apply 実行中である
- **AND** 1回目の apply がエージェントのサマリーを返している
- **WHEN** 2回目の apply が実行される
- **THEN** プロンプトは base apply_prompt を含む
- **AND** プロンプトは `<last_apply attempt="1">` ブロックを含む
- **AND** ブロックには 1回目のサマリーが含まれる

### Requirement: Archive Context History

オーケストレータは、逐次/並列のどちらの archive でも共通ループで同一の履歴注入ロジックを使用し、各 archive 試行の結果をキャプチャして同じ change に対する後続の archive プロンプトに含めなければならない（MUST）。

#### Scenario: 初回 archive 試行には履歴がない
- **WHEN** オーケストレータが change に対して初めて archive を実行する
- **THEN** プロンプトには設定からの基本 archive_prompt のみが含まれる
- **AND** `<last_archive>` タグは含まれない

#### Scenario: 2回目の archive には前回の試行結果が含まれる
- **GIVEN** change に対する archive の1回目の試行が検証失敗した
- **WHEN** オーケストレータが同じ change に対して2回目の archive を実行する
- **THEN** プロンプトには基本 archive_prompt が含まれる
- **AND** プロンプトには `<last_archive attempt="1">` ブロックが含まれる
- **AND** ブロックには試行回数、成功/失敗ステータス、所要時間、検証結果が含まれる

#### Scenario: 複数の前回試行が含まれる
- **GIVEN** change に対する archive が2回失敗している
- **WHEN** オーケストレータが同じ change に対して3回目の archive を実行する
- **THEN** プロンプトには `<last_archive attempt="1">` と `<last_archive attempt="2">` の両方のブロックが含まれる
- **AND** 各ブロックにはそれぞれの試行の詳細が含まれる

#### Scenario: 履歴は change 完了時にクリアされる
- **GIVEN** change に対する archive 履歴が存在する
- **WHEN** archive が成功し、change が完全に処理される
- **THEN** その change の archive 履歴はクリアされる
- **AND** 次に同じ change ID が処理される場合、履歴は空の状態から始まる

#### Scenario: parallel の2回目 archive に履歴が含まれる
- **GIVEN** parallel mode で change が archive 実行中である
- **AND** 1回目の archive が検証失敗している
- **WHEN** 2回目の archive が実行される
- **THEN** プロンプトは base archive_prompt を含む
- **AND** プロンプトは `<last_archive attempt="1">` ブロックを含む
- **AND** ブロックには 1回目の試行結果が含まれる
