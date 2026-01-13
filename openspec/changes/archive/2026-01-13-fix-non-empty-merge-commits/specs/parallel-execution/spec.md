## ADDED Requirements

### Requirement: jj Workspace Merging

jj ワークスペースマネージャーは、複数のリビジョンをマージする際、**empty なマージコミット**を作成しなければならない（SHALL）。

マージコミット自体に変更を含んではならない（SHALL NOT）。working copy の変更は、マージコミットの後に作成される新しいコミットで管理されなければならない（SHALL）。

#### Scenario: 複数リビジョンのマージで empty コミットを作成

- **GIVEN** 2つ以上の並列実行されたリビジョンが存在する
- **WHEN** `merge_workspaces()` が呼び出される
- **THEN** システムは `jj new --no-edit` でマージコミットを作成する
- **AND** マージコミットは empty である（変更を含まない）
- **AND** マージコミットの change_id が返される

#### Scenario: マージ後の working copy 管理

- **GIVEN** マージコミットが作成された
- **WHEN** マージが完了する
- **THEN** システムは `jj new <merge_rev>` で新しい working copy コミットを作成する
- **AND** working copy の未コミット変更は新しいコミットに引き継がれる
- **AND** マージコミット自体は empty のまま保たれる

#### Scenario: マージコミットの empty 状態検証

- **GIVEN** マージコミットが作成された
- **WHEN** `jj log -r <merge_rev> -T 'empty'` を実行する
- **THEN** 出力は `true` である
- **OR** `jj log -r <merge_rev> --summary` を実行したとき、変更のリストが空である

#### Scenario: workspace update-stale の使用禁止

- **GIVEN** マージコミットが作成された
- **WHEN** マージ処理を実行する
- **THEN** システムは `jj workspace update-stale` を呼び出してはならない（SHALL NOT）
- **AND** working copy の状態管理は `jj new` で行われなければならない（SHALL）

