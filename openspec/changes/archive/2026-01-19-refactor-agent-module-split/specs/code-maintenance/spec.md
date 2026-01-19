## ADDED Requirements
### Requirement: Agent モジュールの責務分割
オーケストレーターは Agent の実行・出力処理・履歴管理・プロンプト生成を責務別モジュールに分割し、既存の公開 API と挙動を維持するために SHALL 分割後のモジュール構成を採用しなければならない。

#### Scenario: Agent モジュール構成
- **WHEN** 開発者が Agent モジュールを調査する
- **THEN** runner/output/history/prompt の責務別モジュールが確認できる

#### Scenario: 既存の挙動維持
- **WHEN** 分割後に既存のテストを実行する
- **THEN** すべて成功し、挙動が変わっていないことが確認できる
