# Spec: シェルエスケープ機能

## ADDED Requirements

### Requirement: shlex による安全なトークン生成

POSIX シェル経由で実行するすべてのプレースホルダー展開は、`shlex::try_quote()` を用いて安全なトークンを生成しなければならない (MUST)。

#### Scenario: 特殊文字を含む入力の安全化
- **GIVEN** 特殊文字（`$`, `` ` ``, `!`, `\`）を含む入力が渡される
- **WHEN** `{prompt}` を展開する
- **THEN** シェルで解釈されない安全なトークンとして展開される

#### Scenario: 改行を含む入力の保持
- **GIVEN** 改行を含む入力が渡される
- **WHEN** `{proposal}` を展開する
- **THEN** 改行が保持されつつ安全にクォートされる

### Requirement: テンプレート互換性の維持

`{prompt}`/`{proposal}`/`{conflict_files}` がテンプレート内で既にクォートされている場合でも、二重クォートにならないように互換展開を行わなければならない (MUST)。

#### Scenario: クォート済みテンプレート
- **GIVEN** テンプレートが `"claude '{prompt}'"` の形式である
- **WHEN** `{prompt}` を展開する
- **THEN** 二重クォートされず安全な文字列が埋め込まれる

#### Scenario: クォートなしテンプレート
- **GIVEN** テンプレートが `"claude {prompt}"` の形式である
- **WHEN** `{prompt}` を展開する
- **THEN** `shlex` が生成したトークンがそのまま埋め込まれる

### Requirement: Windows 実行時の安全化

Windows の `cmd /C` 実行経路では POSIX 前提の `shlex` を適用せず、現行挙動を維持した上で危険な文字（NULL など）を除去しなければならない (SHALL)。

#### Scenario: Windows での NULL 文字除去
- **GIVEN** Windows 実行経路で NULL 文字を含む入力が渡される
- **WHEN** `{prompt}` を展開する
- **THEN** NULL 文字を除去した安全な文字列で実行される

### Requirement: コンフリクトファイルの安全な展開

`{conflict_files}` は `shlex` により安全にトークン化され、ファイル名の特殊文字がシェルに解釈されないようにしなければならない (MUST)。

#### Scenario: スペースやクォートを含むファイル名
- **GIVEN** スペースやシングルクォートを含むファイル名が含まれる
- **WHEN** `{conflict_files}` を展開する
- **THEN** すべてのファイル名が安全に解釈される

### Requirement: change_id の安全性検証

`{change_id}` に安全でない文字が含まれていないことを検証し、問題があれば警告またはアサートを発生させなければならない (SHALL)。

#### Scenario: 不正な change_id の検出
- **GIVEN** `{change_id}` に `;` や空白が含まれる
- **WHEN** 展開処理を行う
- **THEN** デバッグ環境で検知できる形で警告またはアサートが発生する

### Requirement: セキュリティテストの実装

コマンドインジェクションを想定した入力を含むテストを追加し、エスケープの安全性を担保しなければならない (MUST)。

#### Scenario: 攻撃パターンの無効化
- **GIVEN** `; rm -rf /` や `$(whoami)` などの入力
- **WHEN** `{prompt}` を展開する
- **THEN** シェルで解釈されない安全なトークンに変換される

### Requirement: expand_prompt() の実装

`expand_prompt()` は `shlex::try_quote()` を用いた実装でなければならない (MUST)。

#### Scenario: 既存実装からの移行
- **GIVEN** 手動エスケープ実装
- **WHEN** shlex ベースに置き換える
- **THEN** 既存テストがパスし、エッジケースの安全性が向上する

### Requirement: expand_proposal() の実装

`expand_proposal()` は `shlex::try_quote()` を用いた実装でなければならない (MUST)。

#### Scenario: 提案文字列の安全化
- **GIVEN** 改行や特殊文字を含む提案文字列
- **WHEN** `{proposal}` を展開する
- **THEN** コマンドインジェクションの危険が排除される
