# CLI Spec Delta: ループコンテキスト履歴

## ADDED Requirements

### Requirement: Archive Context History

オーケストレータは、各 archive 試行の結果をキャプチャし、同じ change に対する後続の archive プロンプトに含めなければならない（MUST）。

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

### Requirement: Archive History Context Format

archive 履歴コンテキストは、XML 風のタグ形式で構造化されなければならない（SHALL）。

各試行ブロックは以下の情報を含む：
- `attempt`: 試行回数（1-based）
- `status`: success または failed
- `duration`: 所要時間（秒単位）
- `verification_result`: 検証結果（検証失敗時の理由）
- `error`: エラーメッセージ（失敗時）
- `exit_code`: 終了コード

#### Scenario: 検証失敗時の履歴フォーマット

- **GIVEN** archive コマンドは成功したが検証が失敗した
- **WHEN** 履歴コンテキストがフォーマットされる
- **THEN** ブロックには `status: failed` が含まれる
- **AND** `verification_result` には「Change still exists at openspec/changes/{change_id}」などの具体的な理由が含まれる
- **AND** `exit_code: 0` が含まれる（コマンド自体は成功したため）

#### Scenario: コマンド失敗時の履歴フォーマット

- **GIVEN** archive コマンドが失敗した
- **WHEN** 履歴コンテキストがフォーマットされる
- **THEN** ブロックには `status: failed` が含まれる
- **AND** `error` には終了コードに関する情報が含まれる
- **AND** `exit_code` には非ゼロの値が含まれる

### Requirement: Resolve Continuation Context

resolve コマンドの再試行時、システムは前回の試行結果と継続理由をプロンプトに含めなければならない（MUST）。

#### Scenario: 初回 resolve 試行にはコンテキストがない

- **WHEN** システムが conflict resolution のために resolve を初めて実行する
- **THEN** プロンプトには基本的な VCS 状態とコンフリクト情報のみが含まれる
- **AND** `<resolve_context>` ブロックは含まれない

#### Scenario: 2回目の resolve には前回の結果と継続理由が含まれる

- **GIVEN** resolve の1回目の試行後もコンフリクトが残っている
- **WHEN** システムが2回目の resolve を実行する
- **THEN** プロンプトには `<resolve_context>` ブロックが含まれる
- **AND** ブロックには現在の試行番号（"attempt 2 of 3"）が含まれる
- **AND** 前回の試行の結果（コマンド終了ステータス、検証結果）が含まれる
- **AND** 検証失敗の具体的な理由（"Conflicts still present: src/main.rs"）が含まれる
- **AND** 所要時間が含まれる

#### Scenario: マージ未完了による継続理由

- **GIVEN** resolve コマンドが成功終了した
- **AND** しかし `MERGE_HEAD` が存在する（マージ未完了）
- **WHEN** システムが次回の resolve を実行する
- **THEN** `<resolve_context>` に「Merge still in progress (MERGE_HEAD exists)」という理由が含まれる

#### Scenario: マージコミット不足による継続理由

- **GIVEN** resolve コマンドが成功終了した
- **AND** しかし必要なマージコミット（"Merge change: {change_id}"）が不足している
- **WHEN** システムが次回の resolve を実行する
- **THEN** `<resolve_context>` に「Missing merge commits for change_ids」という理由が含まれる
- **AND** 不足している change_id のリストが含まれる

#### Scenario: Worktree マージ未完了による継続理由

- **GIVEN** 並列実行モードで resolve コマンドが成功終了した
- **AND** しかし worktree でマージが未完了（worktree に `MERGE_HEAD` が存在）
- **WHEN** システムが次回の resolve を実行する
- **THEN** `<resolve_context>` に「Worktree merge still in progress for '{revision}'」という理由が含まれる

#### Scenario: Pre-sync コミットサブジェクト不正による継続理由

- **GIVEN** 並列実行モードで resolve コマンドが成功終了した
- **AND** しかし pre-sync マージコミットのサブジェクトが期待と異なる
- **WHEN** システムが次回の resolve を実行する
- **THEN** `<resolve_context>` に「Invalid pre-sync merge commit subject」という理由が含まれる
- **AND** 期待されるサブジェクトと実際のサブジェクトが含まれる

### Requirement: Resolve Context Format

resolve コンテキストは、人間とAIが読みやすい形式で構造化されなければならない（SHALL）。

コンテキストブロックには以下が含まれる：
- 現在の試行番号と最大試行回数
- 前回の試行の詳細（コマンド終了ステータス、検証結果、所要時間）
- 検証失敗の具体的な理由（継続理由）
- ループ継続の説明

#### Scenario: コンテキストの可読性

- **WHEN** resolve コンテキストがフォーマットされる
- **THEN** 「This is attempt X of Y」という形式で試行回数が示される
- **AND** 「Previous attempt (N):」というラベルで前回の結果が示される
- **AND** 「Continue resolving...」などの指示が含まれる

## MODIFIED Requirements

### Requirement: Apply History Context Format

apply 履歴コンテキストは、archive と resolve の履歴フォーマットと一貫性を持たなければならない（SHALL）。

#### Scenario: 履歴フォーマットの一貫性

- **GIVEN** システムが apply、archive、resolve の履歴を管理する
- **WHEN** 各履歴がフォーマットされる
- **THEN** すべての履歴で XML 風のタグ形式が使用される
- **AND** すべての履歴で `attempt`、`status`、`duration` フィールドが含まれる
- **AND** 各操作固有の追加情報（`error`、`verification_result`、`continuation_reason`）も含まれる
