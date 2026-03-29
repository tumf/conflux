## MODIFIED Requirements

### Requirement: テンプレート互換性の維持

`{prompt}`/`{proposal}`/`{conflict_files}` がテンプレート内で既にクォートされている場合でも、二重クォートにならないように互換展開を行わなければならない (MUST)。この互換ルールは orchestrator 側の command template だけでなく、server mode の `resolve_command` にも適用されなければならない (MUST)。

#### Scenario: クォート済みテンプレート
- **GIVEN** テンプレートが `"claude '{prompt}'"` の形式である
- **WHEN** `{prompt}` を展開する
- **THEN** 二重クォートされず安全な文字列が埋め込まれる

#### Scenario: クォートなしテンプレート
- **GIVEN** テンプレートが `"claude {prompt}"` の形式である
- **WHEN** `{prompt}` を展開する
- **THEN** `shlex` が生成したトークンがそのまま埋め込まれる

#### Scenario: server resolve_command also honors quoted prompt templates
- **GIVEN** server mode の `resolve_command` テンプレートが `"opencode run --agent code '{prompt}'"` の形式である
- **WHEN** `git/sync` が multi-line prompt を使って `resolve_command` を実行する
- **THEN** server は二重クォートでコマンド文字列を壊さない
- **AND** prompt 全体が 1 つの安全な引数として渡される
