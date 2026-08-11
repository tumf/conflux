## MODIFIED Requirements

### Requirement: Archived 状態の checkbox 表示

TUI は Changes row の checkbox / execution mark semantics と列配置を、その row が execution candidate かどうか、および reducer が archive 完了を記録済みかどうかに応じて表現しなければならない（SHALL）。

Reducer が archive 完了を記録済みの change、または display status が `archived`、`merged`、`pushed` の change は、checkbox テキストとして `[x]` または `[ ]` を表示してはならない（MUST NOT）。TUI は既存 checkbox と同じ3表示列の空白を描画しなければならない（SHALL）。Archive 完了後に display status が `resolving`、`resolve pending`、または `merge wait` へ進んでも、この空白表示を維持しなければならない（SHALL）。Rejected 状態の change も execution candidate ではなく、以前の execution mark を保持したまま表示してはならない（MUST NOT）。

Changes row は cursor glyph とその専用列を表示してはならず（MUST NOT）、focus は row highlight で示さなければならない（SHALL）。Checkbox 領域の直後には1表示列の区切りを置き、change ID の開始から次の field の開始までを36表示列に固定しなければならない（SHALL）。その内訳は最大35表示列の change ID content と1表示列の field separator とする。Change ID content が35表示列を超える場合はellipsisなしで表示幅境界においてhard truncateし、不足する場合は空白でpadしてcontent領域を35表示列にしなければならない（SHALL）。Unicode wide characterが境界を跨ぐ場合はその文字を含めず、残る列を空白でpadしなければならない（SHALL）。

Reducer が archive 完了を記録済みの行では、Spaceとbulk mark操作はsilent no-opでなければならず（SHALL）、mark hintを表示してはならない（MUST NOT）。Display statusが同じ`resolving`でもarchive未完了の行は、従来どおりexecution markを表示・変更できなければならない（SHALL）。

#### Scenario: rejected 状態では x マークを保持しない

- **GIVEN** TUI が change 一覧を表示している
- **AND** ある change が rejection flow 完了により `rejected` 状態へ遷移した
- **WHEN** 画面が次にレンダリングされる
- **THEN** その change は execution mark なし (`selected = false`) で表示される
- **AND** ステータス表示は `rejected` のままである

#### Scenario: archive完了後にresolvingへ進んでもcheckboxを表示しない

- **GIVEN** reducer が change `preserve-archiving-during-tui-refresh` の archive 完了を記録済みである
- **AND** そのchangeのdisplay statusがpost-archive処理により`resolving`である
- **WHEN** Changes listがSelectまたはRunning layoutでレンダリングされる
- **THEN** checkbox領域に`[x]`も`[ ]`も表示されない
- **AND** checkboxと同じ3表示列の空白が表示される
- **AND** change IDと後続fieldの開始位置は非terminal行と同じ列に維持される

#### Scenario: refresh後もarchive完了表示を維持する

- **GIVEN** reducer があるchangeのarchive完了を記録済みである
- **AND** TUIがそのchangeを`resolve pending`または`merge wait`として表示している
- **WHEN** catalog refreshがChange rowを再構築する
- **THEN** reducer由来のarchive完了factが再適用される
- **AND** checkbox textは1 frameも再表示されない

#### Scenario: post-archive 行の Space は silent no-op

- **GIVEN** reducer がcursor rowのarchive完了を記録済みである
- **AND** rowのdisplay statusが`resolving`、`resolve pending`、`merge wait`、`merged`、または`pushed`である
- **WHEN** ユーザーがSpaceまたはbulk mark操作を実行する
- **THEN** execution mark、queue intent、runtime state、および表示状態は変化しない
- **AND** mark hintとmark refusal warningは表示されない

#### Scenario: pre-archive resolving row remains markable

- **GIVEN** changeがarchive完了前のactive `resolving`状態である
- **WHEN** Changes listがレンダリングされ、ユーザーがSpaceを押す
- **THEN** rowは現在のexecution markに応じて`[x]`または`[ ]`を表示する
- **AND** execution markは従来どおり切り替わる

#### Scenario: cursor glyphなしでfocusを表示する

- **GIVEN** Changes listに複数のchange rowがある
- **WHEN** ユーザーがcursorを移動する
- **THEN** どのrowにも`►`またはcursor専用列は表示されない
- **AND** focused rowの既存highlightが移動する

#### Scenario: ASCII change ID field is fixed to 36 columns

- **GIVEN** `fix-stale-resolve-terminal-status`と`preserve-archiving-during-tui-refresh`をChanges listに表示する
- **WHEN** rowがレンダリングされる
- **THEN** 前者のID contentは33表示列と2表示列のpaddingになる
- **AND** 後者は`preserve-archiving-during-tui-refre`へ35表示列でhard truncateされる
- **AND** それぞれの次fieldはID開始位置から36表示列後の同じ列で始まる
- **AND** truncation suffixは表示されない

#### Scenario: wide-character change ID preserves following columns

- **GIVEN** 35表示列の境界付近にUnicode wide characterを含むchange IDがある
- **WHEN** rowがレンダリングされる
- **THEN** wide characterは途中で分割されない
- **AND** contentが35表示列未満になった残りは空白でpadされる
- **AND** 次fieldはASCII ID rowと同じ列で始まる

#### Scenario: narrow terminal omits preview safely

- **GIVEN** fixed Changes row fieldsを表示できるが既存minimum preview widthを残せないterminal幅である
- **WHEN** rowがレンダリングされる
- **THEN** fixed fieldsは同じ列契約で表示される
- **AND** previewは切り詰められたfieldへ重ならず省略される

<!-- Expected canonical result after archive: `cli` will define reducer-derived post-archive checkbox suppression, highlight-only row focus, and a Unicode-aware 35-column ID content field plus one separator column, while removing stale archived `[x]` scenarios. -->

### Requirement: Rejected terminal row の execution mark クリア

TUI は `rejected` terminal row を execution candidate として扱ってはならない（SHALL NOT）。
`ChangeRejected` 遷移を受けた行は `selected=false` へ遷移し、他 change の execution mark は保持しなければならない（SHALL）。Archived rowのcheckbox表示とpost-archive mark admissionは「Archived 状態の checkbox 表示」requirementが所有し、このrequirementはarchived `[x]` stylingを要求してはならない（MUST NOT）。

#### Scenario: rejected transition clears only target mark

- **GIVEN** change `foo` と `bar` が execution mark 付きで queued 表示である
- **WHEN** `foo` が `ChangeRejected` で `rejected` に遷移する
- **THEN** `foo` の execution mark は clear される
- **AND** `bar` の execution mark は保持される

#### Scenario: archived styling is not part of rejected mark clearing

- **GIVEN** TUIがrejected mark clearingとarchived rowの両方を表示する
- **WHEN** Changes listがレンダリングされる
- **THEN** rejected rowだけが`ChangeRejected`により`selected=false`へ遷移する
- **AND** archived rowのcheckbox text、placeholder、mark admissionは「Archived 状態の checkbox 表示」requirementに従う
- **AND** archived `[x]` stylingは要求されない

<!-- Expected canonical result after archive: `cli` will keep rejected mark clearing scoped to rejected rows and remove obsolete archived `[x]` scenarios from this requirement. -->
