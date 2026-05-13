## MODIFIED Requirements

### Requirement: リファクタリング安全性の担保

オーケストレーターはリファクタリング後も既存仕様の挙動を保ち、検証手順で後退がないことを示すために SHALL 検証を通過しなければならない。タスク進捗解決のリファクタリングでは、worktree/base/archive の探索順序と acceptance follow-up の更新結果を characterization test で固定しなければならない。

#### Scenario: タスク進捗解決の探索順序が維持される

- **GIVEN** worktree active、worktree archive、base archive、base active の各 `tasks.md` 候補が存在する
- **WHEN** タスク進捗を fallback 付きで解析する
- **THEN** 既存と同じ優先順位で最初に見つかった `tasks.md` が使われる
- **AND** 解析された完了数と総数はリファクタリング前後で同じである

#### Scenario: acceptance follow-up の更新挙動が維持される

- **GIVEN** acceptance follow-up section を含む、または含まない `tasks.md` が存在する
- **WHEN** acceptance failure findings を記録する
- **THEN** 既存 section は同じ見出し単位で置換される
- **AND** 空 findings の場合は既定の未完了タスクが追加される
