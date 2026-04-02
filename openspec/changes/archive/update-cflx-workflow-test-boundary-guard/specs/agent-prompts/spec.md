## MODIFIED Requirements

### Requirement: Prompts MUST apply a mock-first external dependency policy

AI が単独で解決・検証できない要件は外部依存として扱われなければならない（MUST）。
外部依存がモック/スタブ/フィクスチャで代替可能な場合、プロンプトはそれらの実装を優先し、外部資格情報なしで検証できる状態へ収束させなければならない（MUST）。
apply 側のプロンプトは、unit test を追加・更新する際に stateful external boundary へ直接依存させてはならない（MUST NOT）。
stateful external boundary の例には、VCS/SCM、network/API、database、real filesystem state、real OS process/CLI tool、clock/sleep/timer、環境依存の権限・credential・OS state が含まれる（MUST）。
ロジック中心の検証では、apply プロンプトは decision logic を helper / trait / interface / pure function / in-memory fake へ分離し、実境界ではなく test double で unit test するよう指示しなければならない（MUST）。
real external boundary を必要とする検証は unit test として完了扱いしてはならず、integration test または e2e test として分類しなければならない（MUST）。
unit-test coverage を主張する tasks.md の項目は、追加・更新されたテストが genuinely unit-scoped であり、real external boundary に依存していない場合にのみ完了扱いにできる（MUST）。

#### Scenario: apply が unit test 用ロジックを実境界から分離する
- **GIVEN** apply-mode agent が branching logic や decision logic を検証する task を実装している
- **WHEN** その検証が real git、real process、real filesystem、real network、または real timer なしでも成立する
- **THEN** apply prompt は helper や trait、mock/fake/in-memory fake を使った unit test を優先させる
- **AND** 実境界依存を unit test 完了の根拠として扱わない

#### Scenario: 実境界が必要なテストを unit test 完了に使わない
- **GIVEN** tasks.md に unit test coverage を求める項目がある
- **WHEN** apply-mode agent が追加したテストが real git repo、real CLI process、real filesystem state、database、network、または timer に依存する
- **THEN** apply prompt はそのテストを unit test として完了扱いしない
- **AND** integration/e2e へ再分類するか、pure logic を抽出して別の unit test を追加するよう指示する

### Requirement: Acceptance prompt MUST flag unit-test classification mismatches

acceptance プロンプトは、unit test として説明・配置・完了扱いされたテストが real external boundary に依存していないか確認しなければならない（MUST）。
unit test の主張と実際の test scope が一致しない場合、acceptance は classification mismatch として finding を記録しなければならない（MUST）。
その mismatch によって tasks.md の完了主張が不 truthful になる場合、acceptance は FAIL を出し、pure helper への抽出または integration/e2e への再分類を follow-up として要求しなければならない（MUST）。
明らかな mismatch の例には、unit test と称しながら real git repo を作成する、real process/CLI を起動する、real filesystem/database/network/timer に依存する、または module-local unit test 配置にもかかわらず実質的に integration flow を通すケースが含まれる（MUST）。

#### Scenario: acceptance が unit test と integration test の分類不一致を指摘する
- **GIVEN** acceptance が change のテスト追加内容と tasks.md の完了状態を確認している
- **WHEN** unit test として説明または完了扱いされたテストが real external boundary に依存している
- **THEN** acceptance prompt は classification mismatch finding を記録する
- **AND** pure logic 抽出による unit test 化または integration/e2e への再分類を follow-up として要求する

#### Scenario: classification mismatch が false completion を生む場合は FAIL する
- **GIVEN** tasks.md が unit-test coverage の完了を主張している
- **WHEN** acceptance が確認すると実際には integration-style test しか存在しない
- **THEN** acceptance は FAIL を出力する
- **AND** finding で unsupported な checklist claim を明示する
