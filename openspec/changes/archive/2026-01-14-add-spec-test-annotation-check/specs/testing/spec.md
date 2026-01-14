## MODIFIED Requirements

### Requirement: 仕様とテストのマッピングドキュメント

プロジェクトは仕様シナリオとテストケースの対応関係を、文書だけでなくテストコード上のアノテーションでも表現できなければならない（SHALL）。

アノテーションは、仕様ファイルのパスと、`Requirement` / `Scenario` 見出しから生成した slug を用いて参照できなければならない（SHALL）。

#### Scenario: テストアノテーションから仕様シナリオ参照を検証する

- **GIVEN** `openspec/specs/<capability>/spec.md` に `### Requirement:` と `#### Scenario:` が定義されている
- **AND** テストコードに `// OPENSPEC: openspec/specs/<capability>/spec.md#<req_slug>/<scenario_slug>` が付与されている
- **WHEN** 仕様↔テスト対応付けチェッカーが実行される
- **THEN** 参照先の Requirement/Scenario が存在する場合、参照は有効として扱われる
- **AND** 参照先が存在しない場合、壊れた参照としてレポートされる

#### Scenario: UI-only シナリオは不足検出の対象外

- **GIVEN** ある仕様シナリオが本文中に `UI-only` を含む
- **WHEN** 仕様↔テスト対応付けチェッカーが実行される
- **THEN** 当該シナリオはテスト参照が無くても不足としてレポートされない
