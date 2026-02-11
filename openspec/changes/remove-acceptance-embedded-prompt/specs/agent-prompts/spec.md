## MODIFIED Requirements
### Requirement: Acceptance 固定手順は単一ソースでなければならない
acceptance の固定手順は一箇所に集約されなければならない（MUST）。
固定手順を OpenCode コマンドテンプレート（例: `.opencode/commands/cflx-accept.md`）に置く場合、オーケストレーターは `{prompt}` に固定手順を含めず、可変コンテキストのみを渡さなければならない（MUST）。
acceptance の埋め込みシステムプロンプトは使用してはならず（MUST NOT）、固定手順はコマンドテンプレートからのみ供給される（MUST）。
acceptance_prompt_mode の `full` は互換エイリアスとして扱い、`context_only` と同じ挙動になる（MUST）。

#### Scenario: cflx-accept を使用する場合は context_only を採用する
- **GIVEN** acceptance_command が `/cflx-accept {change_id} {prompt}` を使用する
- **WHEN** acceptance プロンプトを構築する
- **THEN** `{prompt}` は change_id とパス、diff/履歴などの可変コンテキストのみを含む
- **AND** 固定の acceptance 手順は `.opencode/commands/cflx-accept.md` のみから供給される

#### Scenario: full 指定でも固定手順は注入されない
- **GIVEN** acceptance_prompt_mode が `full` に設定されている
- **WHEN** acceptance プロンプトを構築する
- **THEN** 埋め込みの固定手順は注入されない
- **AND** `context_only` と同じ可変コンテキストのみが `{prompt}` に含まれる
