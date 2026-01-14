## ADDED Requirements

### Requirement: `+` による提案作成フローの起動

TUIはSelectモードで `+` キーを押したとき、以下の条件をすべて満たす場合に限り、提案作成フローを開始しなければならない（SHALL）。

- 現在の作業ディレクトリが Git リポジトリ上である
- 設定で `worktree_command` が定義されている

提案作成フローでは、一時ディレクトリ配下に Git worktree を作成し、その worktree を **子プロセスの `cwd`** として `worktree_command` を実行しなければならない（SHALL）。

#### Scenario: Gitリポジトリ上で `worktree_command` が設定されている

- **GIVEN** TUIがSelectモードである
- **AND** 現在の作業ディレクトリがGitリポジトリ上である
- **AND** 設定で `worktree_command` が定義されている
- **WHEN** ユーザーが `+` キーを押す
- **THEN** 一時ディレクトリ配下に新しいGit worktreeが作成される
- **AND** `worktree_command` が作成したworktreeを `cwd` として実行される
- **AND** 作成したworktreeは削除されずに残る

#### Scenario: Gitリポジトリ上でない場合は無操作

- **GIVEN** TUIがSelectモードである
- **AND** 現在の作業ディレクトリがGitリポジトリ上でない
- **WHEN** ユーザーが `+` キーを押す
- **THEN** 何も起こらない

#### Scenario: `worktree_command` 未設定の場合は無操作

- **GIVEN** TUIがSelectモードである
- **AND** 現在の作業ディレクトリがGitリポジトリ上である
- **AND** `worktree_command` が設定されていない
- **WHEN** ユーザーが `+` キーを押す
- **THEN** 何も起こらない

### Requirement: Runningモードでは提案作成不可

TUIはRunningモードで `+` キーを押した場合、何も起こしてはならない（SHALL NOT）。

#### Scenario: Runningモードでは無操作

- **GIVEN** TUIがRunningモードである
- **WHEN** ユーザーが `+` キーを押す
- **THEN** 何も起こらない

## REMOVED Requirements

### Requirement: 提案入力モード

**Reason**: `+` の役割を「提案本文の手入力」から「worktree上での提案作成コマンド起動」に変更するため。

**Migration**: `+` を起点に提案作成を行う場合、設定で `worktree_command` を定義する。

### Requirement: 複数行テキスト入力

**Reason**: Proposing（テキスト入力UI）を廃止するため。

### Requirement: CJK文字幅対応

**Reason**: Proposing（テキスト入力UI）を廃止するため。

### Requirement: 提案入力の確定とキャンセル

**Reason**: Proposing（テキスト入力UI）を廃止するため。

### Requirement: propose_command の設定

**Reason**: `+` の挙動として `propose_command` を利用しないため。

**Migration**: `propose_command` を利用していたフローは `worktree_command` を用いるフローへ移行する。

### Requirement: コマンド実行とログ表示

**Reason**: Proposing（テキスト入力UI）を廃止し、`worktree_command` に置き換えるため。

### Requirement: キーヒントの表示

**Reason**: Proposingモード自体を廃止するため。
