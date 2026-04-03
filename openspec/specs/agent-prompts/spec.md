# agent-prompts Specification

## Purpose

This specification defines the behavior and constraints for AI agent system prompts, particularly the apply prompt (`APPLY_SYSTEM_PROMPT`), to ensure reliable and autonomous task execution.
## Requirements
### Requirement: Apply system prompt MUST include task format guidance
apply プロンプトは tasks.md のフォーマット修正と進捗更新の指示を含めなければならない（MUST）。Future Work / Out of Scope / Notes セクションへタスクを移動する際は、チェックボックス（`- [ ]` または `- [x]`）を削除し、プレーンテキストまたはチェックボックスなしのリスト項目として記載しなければならない（MUST）。WIP スナップショット作成を妨げないため、apply プロンプトは `--no-verify` を一律禁止してはならない（MUST NOT）。

#### Scenario: apply プロンプトが `--no-verify` を一律禁止しない
- **GIVEN** apply プロンプトを生成する
- **WHEN** 進捗スナップショットの作成を行う
- **THEN** プロンプトに `--no-verify` の一律禁止が含まれない

#### Scenario: Future Work へ移動したタスクのチェックボックスを除去する
- **GIVEN** tasks.md に人間作業のタスクがある
- **WHEN** エージェントがタスクを Future Work / Out of Scope / Notes セクションへ移動する
- **THEN** タスクはチェックボックスなしで記載される（例: `2.2 手動確認タスク` または `- 2.2 手動確認タスク`）
- **AND** task_parser はそのタスクを進捗計算に含めない

### Requirement: Apply system prompt MUST enforce non-interactive iteration

The apply system prompt (`APPLY_SYSTEM_PROMPT`) MUST explicitly state that the agent cannot ask questions to the user and must continue working until MaxIteration is reached, making autonomous decisions under operational constraints.

#### Scenario: Continue iteration without asking questions

**Given:**
- apply execution encounters an uncertain decision point

**When:**
- apply agent processes tasks

**Then:**
- Agent does not ask questions to the user
- Agent makes best autonomous decision and proceeds
- Agent continues iteration until MaxIteration is reached

### Requirement: Future Work restrictions MUST be strictly enforced
Future Work への移動は、**人間の作業**、**外部システムのデプロイ/承認**、または**長時間待機が必要な検証**に限って許可されなければならない（MUST）。

面倒さ、難易度、テストの手間、回帰リスクなどを理由に Future Work へ移動してはならない（MUST NOT）。

また、外部依存が **モック/スタブ/フィクスチャで代替可能**な場合は Future Work に移動してはならず（MUST NOT）、モック等の実装によって自動検証可能にしなければならない（MUST）。
真に非モック可能な外部依存のみ Out of Scope / Future Work へ移動でき、その際はチェックボックスを付けてはならない（MUST NOT）。

#### Scenario: 人間作業や外部作業のみ Future Work へ移動する
- **GIVEN** tasks.md に人間作業や外部デプロイが必要なタスクがある
- **AND** tasks.md に難易度が高いが自動化可能なタスクがある
- **WHEN** apply エージェントがタスクの扱いを判断する
- **THEN** 人間作業や外部デプロイのタスクのみ Future Work に移動する
- **AND** 自動化可能なタスクは Future Work に移動しない

#### Scenario: モック可能な外部依存は Future Work に移動せずモック実装を優先する
- **GIVEN** tasks.md に外部依存を含むタスクがある
- **AND** 外部依存はモック/スタブ/フィクスチャで代替可能である
- **WHEN** apply エージェントがタスクの扱いを判断する
- **THEN** そのタスクは Future Work に移動されない
- **AND** モック/スタブ/フィクスチャの実装タスクと検証タスクが優先される

### Requirement: Acceptance MUST fail if excluded sections contain checkboxes
acceptance プロンプトは、Future Work / Out of Scope / Notes セクション内にチェックボックス（`- [ ]` または `- [x]`）が残っている場合、FAIL を出力し apply フェーズに戻さなければならない（MUST）。

#### Scenario: Future Work にチェックボックスが残っていたら FAIL
- **GIVEN** tasks.md の Future Work セクションに `- [ ] タスク` または `- [x] タスク` が存在する
- **WHEN** acceptance フェーズが実行される
- **THEN** acceptance は FAIL を出力する
- **AND** FINDINGS に「Future Work セクションにチェックボックスが残っている」旨を記載する
- **AND** apply フェーズに戻り、チェックボックスの削除が行われる

### Requirement: Acceptance prompt MUST instruct tasks.md follow-up updates on FAIL
acceptance プロンプトは、FAIL を出力する場合に `openspec/changes/{change_id}/tasks.md` を直接更新する手順を明記しなければならない（MUST）。
指示には、`## Acceptance #<n> Failure Follow-up` セクションの追加（または既存セクションの更新）、`- [ ] <finding>` の 1 行 1 finding 形式、`ACCEPTANCE:`/`FINDINGS:` 行を tasks.md に追加しないことを含めなければならない（MUST）。
`<n>` は tasks.md 内の既存の `Acceptance #<n> Failure Follow-up` を基準に決定するよう指示しなければならない（MUST）。

#### Scenario: Acceptance prompt guides follow-up authoring
- **GIVEN** acceptance プロンプトが生成される
- **WHEN** エージェントが FAIL を出力する必要がある
- **THEN** プロンプトに tasks.md の follow-up 追記手順が含まれる
- **AND** `ACCEPTANCE:` や `FINDINGS:` を tasks.md に追加しない指示が含まれる

### Requirement: Acceptance MUST fail when git working tree is dirty
acceptance プロンプトは Git の作業ツリーが完全にクリーンであることを確認しなければならない（MUST）。この確認では `git status --porcelain` を使用し、出力が空であることを前提とする。未コミット変更または未追跡ファイルが存在する場合、acceptance は FAIL を出力し、FINDINGS に該当ファイルのパスを列挙しなければならない（MUST）。

#### Scenario: 未コミット変更または未追跡ファイルがある場合に FAIL する
- **GIVEN** acceptance フェーズが実行される
- **AND** `git status --porcelain` の出力に変更済みファイルまたは未追跡ファイルが含まれる
- **WHEN** acceptance が判定を行う
- **THEN** acceptance は FAIL を出力する
- **AND** FINDINGS に未コミット変更と未追跡ファイルのパスを明記する

### Requirement: acceptance プロンプトは差分コンテキストを提示する
acceptance プロンプトは `<acceptance_diff_context>` ブロックで差分レビュー対象を提示しなければならない（MUST）。初回は base branch と現在コミットの差分ファイル一覧を含め、2回目以降は前回 acceptance のコミットからの差分ファイルと前回 findings を含める（MUST）。

#### Scenario: 初回 acceptance で base 差分を提示する
- **GIVEN** acceptance 初回で base branch が判定できる
- **WHEN** acceptance プロンプトを構築する
- **THEN** `<acceptance_diff_context>` に base branch → 現在コミットの変更ファイル一覧が含まれる

#### Scenario: 2回目以降は前回 acceptance からの差分と findings を提示する
- **GIVEN** acceptance の過去試行が存在する
- **WHEN** acceptance プロンプトを構築する
- **THEN** `<acceptance_diff_context>` に前回 acceptance からの差分ファイルと previous findings が含まれる

### Requirement: acceptance システムプロンプトは差分レビューの優先指示を含める
acceptance システムプロンプトは、`<acceptance_diff_context>` が存在する場合に変更ファイルの確認を優先するよう明示的に指示しなければならない（MUST）。

#### Scenario: diff context を優先レビューする指示
- **GIVEN** `<acceptance_diff_context>` がプロンプトに含まれる
- **WHEN** acceptance が検証手順を実行する
- **THEN** 変更ファイルの確認を優先する指示が含まれる

### Requirement: Prompts MUST apply a mock-first external dependency policy

AI が単独で解決・検証できない要件は外部依存として扱われなければならない（MUST）。
外部依存がモック/スタブ/フィクスチャで代替可能な場合、プロンプトはそれらの実装を優先し、外部資格情報なしで検証できる状態へ収束させなければならない（MUST）。

#### Scenario: モック可能な外部依存をモック化して自己完結の検証へ導く
- **GIVEN** tasks.md に外部 API 連携が含まれる
- **AND** API 連携はモック/スタブ/フィクスチャで代替可能である
- **WHEN** proposal/apply/acceptance のいずれかのプロンプトが次アクションを指示する
- **THEN** モック/スタブ/フィクスチャの実装と、それに基づく検証（テスト等）を優先する指示が含まれる
- **AND** 外部資格情報（本番キー等）の要求を前提にしない

### Requirement: Missing secrets MUST NOT be treated as a reason to CONTINUE

プロンプトは、秘密情報（API キー等）の欠如を CONTINUE の理由として扱ってはならない（MUST NOT）。
秘密情報が必要な検証が残っている場合、acceptance は FAIL を出力し、モック/スタブ/フィクスチャの実装、または非モック可能である旨の Out of Scope への移動を、具体的な follow-up タスクとして落とし込まなければならない（MUST）。

#### Scenario: API キー欠如を検出したら FAIL としてスタブ実装タスクへ誘導する
- **GIVEN** acceptance が検証を実行しようとする
- **AND** 外部 API の資格情報が未設定である
- **WHEN** acceptance が判定を行う
- **THEN** acceptance は CONTINUE ではなく FAIL を出力する
- **AND** follow-up に「スタブ/フィクスチャの実装」または「非モック可能として Out of Scope へ移動（チェックボックスなし）」が含まれる

### Requirement: Acceptance prompt MUST support sub-agent parallel verification with a single final verdict
acceptance プロンプトは、独立した検証項目をサブエージェントに分割して並列実行し、親エージェントが統合して最終判定を 1 回だけ出力する手順を含めなければならない（MUST）。サブエージェントは `ACCEPTANCE:` を出力してはならない（MUST NOT）。サブエージェントの出力は親が統合可能な構造（例: JSON または見出し + 根拠の箇条書き）であることを要求しなければならない（MUST）。

#### Scenario: サブエージェントの結果を統合して 1 回だけ判定する
- **GIVEN** acceptance プロンプトが生成される
- **WHEN** サブエージェント分割が可能な環境で acceptance を実行する
- **THEN** 親エージェントのみが `ACCEPTANCE:` を 1 回だけ出力する
- **AND** サブエージェントは構造化された結果のみを返す

### Requirement: Acceptance prompt MUST enforce the same scope constraints for sub-agents
acceptance プロンプトは、サブエージェントにも change_id と paths によるスコープ制約を適用し、指定された change 以外の `openspec/changes/**` をレビューしないよう明示しなければならない（MUST）。

#### Scenario: サブエージェントが指定 change のみをレビューする
- **GIVEN** acceptance プロンプトが change_id と paths を提供している
- **WHEN** サブエージェントが検証を実行する
- **THEN** 指定された change 以外のファイルをレビューしない

### Requirement: Acceptance prompt MUST define a sequential fallback when sub-agents are unavailable
acceptance プロンプトは、サブエージェントが利用できない場合に同等のチェックを逐次で実行するフォールバック手順を含めなければならない（MUST）。

#### Scenario: サブエージェントが利用できない場合の逐次実行
- **GIVEN** サブエージェントが利用できない環境で acceptance を実行する
- **WHEN** acceptance プロンプトに従って検証を開始する
- **THEN** 同等のチェックを逐次で完了する手順が提示される

### Requirement: Acceptance 固定手順は単一ソースでなければならない
acceptance の固定手順は一箇所に集約されなければならない（MUST）。
固定手順を OpenCode コマンドテンプレート（例: `.opencode/commands/cflx-accept.md`）に置く場合、オーケストレーターは `{prompt}` に固定手順を含めず、可変コンテキストのみを渡さなければならない（MUST）。
acceptance の埋め込みシステムプロンプトは使用してはならず（MUST NOT）、固定手順はコマンドテンプレートからのみ供給される（MUST）。
acceptance_prompt_mode の `full` は互換エイリアスとして扱い、`context_only` と同じ挙動になる（MUST）。

#### Scenario: cflx-accept を使用する場合は context_only を採用する
- **GIVEN** acceptance_command が `/cflx-accept {change_id} {prompt}` を使用する
- **WHEN** acceptance プロンプトを構築する
- **THEN** `{prompt}` は change_id とパス、diff/履歴などの可変コンテキストのみを含む
- **AND** 固定の acceptance 手順は `.opencode/commands/cflx-accept.md` のみから供給される

#### Scenario: full 指定でも固定手順は注入されない
- **GIVEN** acceptance_prompt_mode が `full` に設定されている
- **WHEN** acceptance プロンプトを構築する
- **THEN** 埋め込みの固定手順は注入されない
- **AND** `context_only` と同じ可変コンテキストのみが `{prompt}` に含まれる

### Requirement: Apply prompt MUST escalate implementation blockers
apply プロンプトは、仕様矛盾や非モック可能な外部制限により実装が不可能と判断した場合、Implementation Blocker を記録してエスカレーションしなければならない（MUST）。

Implementation Blocker の記録は以下を満たさなければならない（MUST）。
- `openspec/changes/{change_id}/tasks.md` に `## Implementation Blocker #<n>` セクションを追加する
- セクション内に「カテゴリ」「根拠（ファイルパス/ログ）」「影響範囲」「解除アクション」を明記する
- セクション内の箇条書きにチェックボックスを付けてはならない（MUST NOT）
- stdout に `IMPLEMENTATION_BLOCKER:` ブロックを出力し、tasks.md と同じ内容を含める

#### Scenario: apply が実装不可を検知して blocker を記録する
- **GIVEN** apply が仕様矛盾または非モック可能な外部制限により実装不可と判断する
- **WHEN** apply がエスカレーションを行う
- **THEN** tasks.md に `## Implementation Blocker #<n>` セクションが追加される
- **AND** セクション内にカテゴリ・根拠・影響範囲・解除アクションが記載される
- **AND** stdout に `IMPLEMENTATION_BLOCKER:` ブロックが出力される

### Requirement: Acceptance prompt MUST evaluate implementation blockers
acceptance プロンプトは Implementation Blocker を審査し、妥当と判断した場合は `ACCEPTANCE: BLOCKED` を出力しなければならない（MUST）。

acceptance は以下を満たさなければならない（MUST）。
- `Implementation Blocker` の内容が不十分または誤りの場合は `ACCEPTANCE: FAIL` を出力し、follow-up タスクを tasks.md に追加する
- `ACCEPTANCE: BLOCKED` の場合は blocker の概要を簡潔に出力する

#### Scenario: acceptance が blocker を承認して BLOCKED を返す
- **GIVEN** tasks.md に妥当な Implementation Blocker が記録されている
- **WHEN** acceptance が blocker を評価する
- **THEN** acceptance は `ACCEPTANCE: BLOCKED` を出力する


#


#


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


### Requirement: acceptance プロンプトは差分コンテキストを提示する

acceptance プロンプトは `<acceptance_diff_context>` ブロックで差分レビュー対象を提示しなければならない（MUST）。初回は base branch と現在コミットの差分ファイル一覧を含め、2回目以降は前回 acceptance のコミットからの差分ファイルと前回 findings を含める（MUST）。また acceptance プロンプトは、レビュー対象が archive へ進む前に final archive commit を阻害する品質ゲートがないか確認する指示を含めなければならない（MUST）。その確認には、リポジトリ標準の final-commit quality gate（pre-commit hook、format、lint、test、またはそれに準ずる documented gate）を使い、archive フェーズで初めて発火する失敗を acceptance で先に露出させなければならない（MUST）。

#### Scenario: 初回 acceptance で base 差分を提示する
- **GIVEN** acceptance 初回で base branch が判定できる
- **WHEN** acceptance プロンプトを構築する
- **THEN** `<acceptance_diff_context>` に base branch → 現在コミットの変更ファイル一覧が含まれる

#### Scenario: 2回目以降は前回 acceptance からの差分と findings を提示する
- **GIVEN** acceptance の過去試行が存在する
- **WHEN** acceptance プロンプトを構築する
- **THEN** `<acceptance_diff_context>` に前回 acceptance コミットからの変更ファイル一覧が含まれる
- **AND** 前回 findings が含まれる

#### Scenario: acceptance prompts archive-readiness verification
- **GIVEN** acceptance プロンプトが archive 前の最終レビューとして生成される
- **WHEN** acceptance が実行される
- **THEN** プロンプトは final archive commit を阻害する quality gate がないか確認するよう指示する
- **AND** その gate failure を単なる後続 archive 問題として見逃さない
