## ADDED Requirements
### Requirement: Acceptance MUST fail when git working tree is dirty
acceptance プロンプトは Git の作業ツリーが完全にクリーンであることを確認しなければならない（MUST）。この確認では `git status --porcelain` を使用し、出力が空であることを前提とする。未コミット変更または未追跡ファイルが存在する場合、acceptance は FAIL を出力し、FINDINGS に該当ファイルのパスを列挙しなければならない（MUST）。

#### Scenario: 未コミット変更または未追跡ファイルがある場合に FAIL する
- **GIVEN** acceptance フェーズが実行される
- **AND** `git status --porcelain` の出力に変更済みファイルまたは未追跡ファイルが含まれる
- **WHEN** acceptance が判定を行う
- **THEN** acceptance は FAIL を出力する
- **AND** FINDINGS に未コミット変更と未追跡ファイルのパスを明記する
