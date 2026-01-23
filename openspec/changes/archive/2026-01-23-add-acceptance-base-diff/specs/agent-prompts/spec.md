## ADDED Requirements

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
