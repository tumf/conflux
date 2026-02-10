## ADDED Requirements
### Requirement: Git hooks ツールの案内
README.md、README.ja.md、DEVELOPMENT.md は Git hooks 管理に prek を使用することを明示し、インストール/フック導入/実行方法を記載しなければならない（SHALL）。
pre-commit のインストール手順は記載してはならない（MUST NOT）。
`.pre-commit-config.yaml` が prek の互換設定として利用されることを明示しなければならない（MUST）。

#### Scenario: prek 導入手順の提示
- **WHEN** 開発者が Git hooks のセットアップ手順を確認する
- **THEN** `prek install` が記載されている
- **AND** `pre-commit uninstall` を含む移行手順が記載されている

#### Scenario: README の Git hooks セクション整合
- **WHEN** README.md と README.ja.md を参照する
- **THEN** Git hooks セクションが両方に存在する
- **AND** コマンド例が一致している
- **AND** `.pre-commit-config.yaml` が互換設定として記載されている

#### Scenario: DEVELOPMENT.md のフック手順
- **WHEN** DEVELOPMENT.md を読む
- **THEN** prek のインストールと実行方法が記載されている
- **AND** `pre-commit install` の記述がない
