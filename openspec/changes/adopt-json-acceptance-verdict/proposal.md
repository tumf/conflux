---
change_type: implementation
priority: high
dependencies: []
references:
  - src/acceptance.rs
  - src/parallel/executor.rs
  - .opencode/commands/cflx-accept.md
  - skills/cflx-accept/SKILL.md
  - skills/cflx-archive/SKILL.md
  - openspec/specs/agent-prompts/spec.md
  - openspec/specs/parallel-execution/spec.md
---

# Proposal: adopt-json-acceptance-verdict

**Change Type**: implementation

## Premise / Context

- 現在の acceptance は canonical verdict を `ACCEPTANCE: PASS|FAIL|CONTINUE|BLOCKED` の単独行として厳密パースしている。
- 実ログでは `ACCEPTANCE: PASS# ...` のように PASS 文言を含みつつ canonical contract を満たさない出力が発生し、1回目 acceptance が `CONTINUE` 扱いになった。
- `opencode run` の実験では、MCP を追加しなくても `stdout` に strict JSON 1 行を返させられ、`--format json` ではイベント JSON から本文を機械取得できることを確認した。
- ユーザは今回の変更では `skills/` 配下の追随修正も必要だと明示している。
- 既存 spec は acceptance verdict を standalone line contract に寄せているが、opencode 実行系の機械可読出力を first-class contract としてはまだ定義していない。

## Problem / Context

acceptance の最終判定を plain-text marker の単独行に依存させると、本文整形や見出し連結、要約付加などの軽微な出力ドリフトで `CONTINUE` に落ちる。

この失敗は「実装が未完了」ではなく「出力フォーマットが機械契約を少し外した」だけでも起こるため、accept→archive handoff の安定性が model 出力の細部に過度に依存している。

また、現在の instruction 群は `.opencode/commands/cflx-accept.md` を canonical source としているが、repo 内 `skills/cflx-accept/` や archive まわりの skill も同じ機械契約を前提に更新しないと、運用ガイドと runtime contract が再びずれる。

## Proposed Solution

acceptance verdict contract を plain text standalone marker 依存から、`opencode run` で返せる strict JSON verdict を優先する形へ更新する。

具体的には以下を行う。

1. acceptance runtime は最終 verdict を JSON から機械判定できるようにし、plain text marker は後方互換 fallback に下げる。
2. acceptance command/template は、最終出力として 1 行の strict JSON verdict を返すよう指示する。
3. `opencode run --format json` を使う場合も使わない場合も、runtime が同じ verdict object を取り出せる contract を定義する。
4. `skills/cflx-accept/` と archive 関連 skill は、新しい verdict contract と fallback 方針を前提に追随更新する。
5. malformed plain text verdict で retry に落ちていた既存ケースを、JSON verdict では安定して accept 完了できるよう regression test で固定する。

## Acceptance Criteria

- acceptance runtime は strict JSON verdict を優先的に解釈し、`pass` / `fail` / `continue` / `blocked` を機械判定できる。
- `opencode run` に strict JSON 1 行を要求した場合、その出力を acceptance handoff に利用できる。
- `--format json` のイベント出力経由でも、本文に含まれる verdict JSON を抽出して同じ結果に正規化できる。
- 旧来の plain text standalone marker は後方互換として残るが、runtime は JSON contract を primary として扱う。
- `skills/cflx-accept/SKILL.md` と archive 側の関連 skill が、新しい verdict contract / fallback / machine-readable expectation を明示する。
- malformed text verdict では CONTINUE になっていたケースに対し、JSON verdict では retry せず archive handoff まで進める回帰テストが追加される。

## Out of Scope

- 新しい MCP server の導入
- apply / archive / resolve 全体を同じ JSON contract に一気に揃えること
- opencode 本体の upstream 実装変更
