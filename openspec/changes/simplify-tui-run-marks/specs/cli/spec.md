## MODIFIED Requirements

### Requirement: Archived 状態の checkbox 表示

TUI は execution mark を、その change が次回 run の候補として保持されている間だけ checkbox として表現しなければならない（SHALL）。

`archived`、`merged`、または `pushed` 状態の change は execution candidate ではないため、checkbox テキストとして `[x]` または `[ ]` を表示してはならない（MUST NOT）。TUI は既存 checkbox と同じ表示幅の空白を描画し、cursor、change ID、badge、status、progress、および preview の開始位置を詰めてはならない（MUST NOT）。

#### Scenario: 実行モードで archived 状態の checkbox を表示しない

- **GIVEN** TUI が実行モードである
- **AND** ある change の display status が `archived`、`merged`、または `pushed` である
- **WHEN** 画面がレンダリングされる
- **THEN** その change の checkbox 領域に `[x]` も `[ ]` も表示されない
- **AND** checkbox と同じ幅の空白が表示される
- **AND** cursor、change ID、badge、status、progress、および preview の開始位置は非 terminal 行と同じ列に維持される

#### Scenario: 選択モードに戻った際も post-archive checkbox は非表示

- **GIVEN** 処理が完了し TUI が選択モードに戻った
- **AND** ある change の display status が `archived`、`merged`、または `pushed` である
- **WHEN** 画面がレンダリングされる
- **THEN** checkbox テキストは表示されない
- **AND** 行の残りの表示位置は詰められない

#### Scenario: post-archive 行の Space は silent no-op

- **GIVEN** cursor が `archived`、`merged`、または `pushed` 行にある
- **WHEN** ユーザーが Space を押す
- **THEN** execution mark、queue intent、runtime state、および表示状態は変化しない
- **AND** mark refusal warning は表示されない

<!-- Expected canonical result after archive: `cli` will replace gray post-archive `[x]` rendering with a fixed-width blank checkbox placeholder and silent Space no-op semantics. -->
