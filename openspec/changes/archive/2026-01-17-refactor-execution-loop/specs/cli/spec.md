## MODIFIED Requirements

### Requirement: Apply Context History

オーケストレーターは、各 apply 試行の最終サマリーメッセージを記録し、同一 change の次回 apply プロンプトに含めなければならない（MUST）。

#### Scenario: parallel の2回目 apply に履歴が含まれる
- **GIVEN** parallel mode で change が apply 実行中である
- **AND** 1回目の apply がエージェントのサマリーを返している
- **WHEN** 2回目の apply が実行される
- **THEN** プロンプトは base apply_prompt を含む
- **AND** プロンプトは `<last_apply attempt="1">` ブロックを含む
- **AND** ブロックには 1回目のサマリーが含まれる
