## MODIFIED Requirements

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
