## Context
apply の WIP スナップショットはスタール検知と再開の根拠になる。pre-commit フックが失敗すると WIP が残らず、進捗が記録されない。

## Goals / Non-Goals
- Goals:
  - WIP スナップショットの作成をフック失敗で阻害しない
  - 影響範囲を WIP スナップショットに限定する
- Non-Goals:
  - apply 成功時の Apply コミットや archive/merge のフック挙動を変えること
  - resolve プロンプトの `--no-verify` 方針を変更すること

## Decisions
- Decision: WIP スナップショットの Git コミットに `--no-verify` を付与する
- Alternatives considered: apply プロンプト禁止を維持しつつ、フックに依存しない WIP 取得手段を追加する
  - Rationale: 追加の仕組みは複雑化するため採用しない

## Risks / Trade-offs
- フックを通さない WIP コミットに不整合が含まれる可能性がある
  - Mitigation: WIP はあくまで作業スナップショットであり、最終 Apply/Archive は通常フックを通す

## Migration Plan
- 既存変更への互換性は維持されるため移行は不要

## Open Questions
- なし
