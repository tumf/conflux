## ADDED Requirements
### Requirement: Acceptance 固定手順は単一ソースでなければならない
acceptance の固定手順は一箇所に集約されなければならない（MUST）。
固定手順を OpenCode コマンドテンプレート（例: `.opencode/commands/cflx-accept.md`）に置く場合、オーケストレーターは `{prompt}` に固定手順を含めず、可変コンテキストのみを渡さなければならない（MUST）。

#### Scenario: cflx-accept を使用する場合は context_only を採用する
- **GIVEN** acceptance_command が `/cflx-accept {change_id} {prompt}` を使用する
- **WHEN** acceptance プロンプトを構築する
- **THEN** `{prompt}` は change_id とパス、diff/履歴などの可変コンテキストのみを含む
- **AND** 固定の acceptance 手順は `.opencode/commands/cflx-accept.md` のみから供給される
