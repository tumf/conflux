## ADDED Requirements

### Requirement: git_sync 回帰テストは共通フィクスチャを利用する

システムは `git/sync` の主要回帰シナリオについて、公開レスポンス契約を固定した characterization test を維持しなければならない（SHALL）。

これらのテストは、router / state / repository divergence の共通フィクスチャを再利用し、公開 API の期待結果を意図中心に検証できる構造でなければならない（MUST）。

#### Scenario: 同期済みシナリオの公開レスポンスが維持される

- **GIVEN** ローカルとリモートが既に一致しているプロジェクトがある
- **WHEN** `POST /api/v1/projects/{id}/git/sync` を呼び出す
- **THEN** 応答の `status` は既存どおりである
- **AND** `resolve_command_ran`、`resolve_exit_code`、`push.status`、`skipped_reason` の契約は変わらない

#### Scenario: 差分ありシナリオでも helper 抽出後に回帰しない

- **GIVEN** local/remote divergence または remote ahead の状態を作る共通 fixture がある
- **WHEN** `git/sync` 回帰テストを実行する
- **THEN** 既存どおりのステータスコードと JSON 契約が検証される
- **AND** テスト本体は重複した準備コードではなく期待挙動の assertion を中心に記述される
