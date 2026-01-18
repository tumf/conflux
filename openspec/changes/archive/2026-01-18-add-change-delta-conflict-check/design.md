## Context
changes配下のspec deltaは複数の変更提案が同じRequirementに対して異なる更新を行う可能性があり、実装やレビューの前に検出できる仕組みが不足している。LLMに依存せず、CLIで静的に衝突を検出する。

## Goals / Non-Goals
- Goals:
  - changes間のspec delta衝突を機械的に検出する
  - CLIから人間向け出力とJSON出力を提供する
  - 衝突検出時の終了コードを定義する
- Non-Goals:
  - 意味的な矛盾の推論（Requirement名が異なる場合の判断）
  - LLMを用いた解釈やレビュー自動化

## Decisions
- Decision: 衝突判定はRequirement名の一致を軸にし、内容の不一致を検出する
  - Why: 誤検知を抑え、機械的に判定できる最小単位に限定するため
- Decision: 解析対象は openspec/changes 配下の非archive changeのみとする
  - Why: 実装前の提案間衝突に焦点を絞るため
- Decision: 衝突出力は人間向けとJSONの2形式を提供する
  - Why: ローカルチェックとCIなど自動処理の両方を支えるため

## Risks / Trade-offs
- Requirement名が一致しない矛盾は検出できない
- 変更の細かな差分ではなくブロック単位で比較するため、意図しない差分として検出される可能性がある

## Open Questions
- 衝突検出結果の既定出力フォーマット（箇条書き/表）の最終形
- 終了コードの詳細（例: 2=衝突検出、1=解析失敗）
