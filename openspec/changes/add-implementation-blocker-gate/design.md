## Context
apply 実行中に仕様矛盾や外部制限が顕在化するケースがある。現状は acceptance が FAIL となり apply ループへ戻るため、実装不可でも反復が続きやすい。apply から「実装不可」をエスカレーションし、acceptance が妥当性を確認して停止するゲートを追加する。

## Goals / Non-Goals
- Goals:
  - 実装不可のエスカレーションを apply から出し、acceptance が最終判定する
  - `BLOCKED` 判定時に当該 change の apply ループを停止する
  - serial/parallel の両方で一貫した停止挙動を提供する
- Non-Goals:
  - 仕様矛盾の自動解決や再提案の自動生成
  - アーカイブ/マージの自動実行
  - 既存の PASS/FAIL/CONTINUE 仕様の破壊的変更

## Decisions
- apply は実装不可と判断した場合、`tasks.md` に `## Implementation Blocker #<n>` セクションを追加し、理由と証拠、解除アクションを記録する
- acceptance は Implementation Blocker を審査し、妥当と判断した場合のみ `ACCEPTANCE: BLOCKED` を返す
- `BLOCKED` 判定は当該 change の apply ループを停止する終端ステータスとして扱う
- 既存の PASS/FAIL/CONTINUE の処理は維持し、BLOCKED を追加語彙として拡張する

## Risks / Trade-offs
- 誤検知によって本来可能な実装が停止するリスクがあるため、acceptance による妥当性審査を必須とする
- ループ停止により tasks.md が未完了のまま残るが、Implementation Blocker により理由を明示する

## Migration Plan
1) acceptance 判定語彙に BLOCKED を追加する
2) apply/acceptance プロンプトに Blocker ルールを追加する
3) serial/parallel の処理フローに BLOCKED 分岐を追加する
4) テストで判定と停止動作を検証する

## Open Questions
- なし
