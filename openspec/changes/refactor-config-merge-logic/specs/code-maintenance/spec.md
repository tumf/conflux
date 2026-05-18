## ADDED Requirements

### Requirement: Config merge refactor preserves precedence

設定 merge ロジックの共通化は、既存の設定探索順、merge priority、`Some`/`None` 上書き規則、hooks deep merge、deprecated helper の後方互換挙動を変更してはならない。

#### Scenario: merge priority が維持される

**Given**: platform、XDG default、XDG env、project、custom の各設定レイヤーがある
**When**: 共通化後の merge ロジックで設定を統合する
**Then**: 高優先レイヤーの `Some` 値が低優先値を上書きする
**And**: 高優先レイヤーの `None` は低優先値を消さない

#### Scenario: 特殊 merge contract が維持される

**Given**: hooks の deep merge と deprecated path helper の後方互換 contract がある
**When**: config merge logic を共通化する
**Then**: hooks の個別フィールド merge 結果は分割前と同等である
**And**: deprecated helper の戻り値と fallback は分割前と同等である
