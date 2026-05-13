## MODIFIED Requirements

### Requirement: リファクタリング安全性の担保

オーケストレーターはリファクタリング後も既存仕様の挙動を保ち、検証手順で後退がないことを示すために SHALL 検証を通過しなければならない。OpenSpec コマンドエンジンの責務分離では、list/show/validate/archive の CLI contract と spec promotion の結果を characterization test で固定しなければならない。

#### Scenario: OpenSpec validate の contract が維持される

- **GIVEN** 妥当な変更提案と不正な変更提案が存在する
- **WHEN** strict validation を実行する
- **THEN** proposal、tasks、spec delta、scenario、change type の必須チェック結果はリファクタリング前と同等である
- **AND** exit code とエラー/警告の分類は同等である

#### Scenario: archive 前 promotion safety が維持される

- **GIVEN** ADDED、MODIFIED、REMOVED、または no-op になる spec delta が存在する
- **WHEN** archive 前の promotion simulation を実行する
- **THEN** canonical spec へ適用可能な delta だけが成功する
- **AND** missing target や no-op promotion は既存と同じ安全側の失敗として扱われる
