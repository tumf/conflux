## ADDED Requirements

### Requirement: Acceptance verdict marker MUST be on its own line
acceptance プロンプトおよびスキル定義は、verdict マーカー（`ACCEPTANCE: PASS`、`ACCEPTANCE: FAIL`、`ACCEPTANCE: CONTINUE`、`ACCEPTANCE: BLOCKED`）を独立した行に出力するよう明示的に指示しなければならない（MUST）。マーカー行にはマーカー文字列以外のテキストを含めてはならない（MUST NOT）。この指示はプロンプトテンプレート、スキル定義（`SKILL.md`）、およびコマンド定義（`.opencode/commands/cflx-accept.md`）のすべてに含めなければならない（MUST）。

#### Scenario: Marker formatting rule is documented in acceptance instructions
- **GIVEN** acceptance の指示がスキル定義またはコマンド定義に記述されている
- **WHEN** エージェントが出力フォーマットのセクションを読む
- **THEN** verdict マーカーが独立した行でなければならない旨の CRITICAL ルールが記載されている
- **AND** 不正なフォーマット例（例: `ACCEPTANCE: PASSAll criteria verified`）と正しいフォーマット例が提示されている
