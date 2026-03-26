## ADDED Requirements

### Requirement: 設定モジュール分割時の挙動互換
システムは設定モジュールを内部的に分割しても、設定の解決結果と公開インターフェースを維持しなければならない。

#### Scenario: 設定優先順位が維持される
- **GIVEN** 同一キーが custom/project/global/default の複数レイヤに存在する
- **WHEN** 設定を読み込む
- **THEN** 優先順位は従来どおり custom > project > global > default で解決される

#### Scenario: 既存CLI挙動が変わらない
- **GIVEN** 既存のCLI実行オプションで設定を読み込む
- **WHEN** リファクタ後のバイナリを実行する
- **THEN** CLIの公開挙動（引数、終了コード、主要出力）は変更されない
