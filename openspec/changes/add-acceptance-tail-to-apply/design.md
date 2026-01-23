## Context
acceptance 失敗後に apply へ戻る場合、acceptance の出力 tail が apply プロンプトに含まれていないため、FINDINGS の内容が apply エージェントに伝わらない。

## Goals / Non-Goals
- Goals:
  - acceptance の stdout/stderr tail を apply の最初の試行へ引き継ぐ
  - tail の内容はそのまま渡し、パースや再構成を行わない
- Non-Goals:
  - acceptance 判定ロジックや findings パーサの変更
  - apply 以外のプロンプト（acceptance/archive）への追加

## Decisions
- Decision: apply プロンプト内に `<last_acceptance_output>` ブロックを追加し、stdout_tail 優先で注入する
- Decision: 注入は acceptance 失敗後の最初の apply 試行のみとし、1 度注入したら同じ acceptance 由来の tail は再注入しない
- Alternatives considered: すべての apply 試行に常時注入（プロンプト肥大化と繰り返しの懸念で不採用）

## Risks / Trade-offs
- プロンプト長が増えるため、tail の長さは現状の stdout/stderr tail に限定する

## Migration Plan
- 既存の acceptance_history に保存されている tail を利用し、追加の永続化は行わない

## Open Questions
- なし
