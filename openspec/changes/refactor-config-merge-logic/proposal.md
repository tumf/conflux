---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/config/mod.rs
  - src/config/types.rs
  - src/config/defaults.rs
  - openspec/specs/code-maintenance/spec.md
---

# Config merge logic の共通化

**Change Type**: implementation

## Problem/Context

設定管理は既に `src/config/` 配下へ分割されているが、`OrchestratorConfig::merge` は多数の `Option` フィールドを個別 `if other.field.is_some()` で上書きしており、設定項目追加時に漏れやすい。`src/config/mod.rs` も 2,282 行あり、path precedence、merge、deprecated helper、server config などのテストが同居しているため、設定 contract の維持をテストで固定した上で merge ロジックを整理する価値が高い。

候補ランキングでは、中心的な設定面でありながら functional change を避けやすく、既存テストも豊富なため、低リスクで保守性向上の見込みがある対象として選定した。

### Evidence

- `src/config/types.rs:661` から `OrchestratorConfig::merge` が多数の `Option` フィールドを手書き分岐で処理している。
- `src/config/types.rs:722` では hooks だけ deep merge の特別処理を持ち、通常フィールドと混在している。
- `src/config/mod.rs:1175` 以降に XDG path precedence のテスト、`src/config/mod.rs:2072` 以降に merge priority characterization があり、設定 contract が重要である。
- `src/config/mod.rs:63` と `src/config/mod.rs:110` に deprecated helper があり、後方互換テストと通常ロジックが密結合している。

## Proposed Solution

- `OrchestratorConfig::merge` の挙動を characterization test で先に固定する。
- `Option` フィールドの「高優先 config の Some だけが上書きする」規則を小さな共通ヘルパーへ集約する。
- hooks や server など、通常上書きと deep merge の違いを明示的に分離する。
- deprecated path helper の後方互換テストは維持しつつ、通常の global path precedence と混ざらない構成へ整理する。

## Acceptance Criteria

- custom/project/XDG env/XDG default/platform/default の merge priority が変わらない。
- `None` は既存値を上書きせず、`Some` は高優先値として上書きする既存規則を維持する。
- hooks の deep merge と server/proposal session などの上書き規則が既存と同等である。
- deprecated helper は後方互換 contract を維持する。
- 設定ファイルの形式、CLI/API contract、デフォルト値は変更しない。

## Explicit Completion Conditions

- `src/config/types.rs` の merge 実装が、通常 Option 上書き、deep merge、特殊互換処理を読み分けられる構造になっている。
- `src/config/mod.rs` または分割後テストで merge priority と path precedence の characterization test が成功している。
- 新しい設定項目を追加する場合に merge 漏れを検出しやすいテストまたは構造がある。
- `cargo fmt --check` と対象 config テストが成功する。

## Out of Scope

- 設定キー名、設定ファイル探索順、デフォルト値、deprecated helper の削除。
- 新しい設定項目の追加。
- `.cflx.jsonc` のスキーマ変更。
