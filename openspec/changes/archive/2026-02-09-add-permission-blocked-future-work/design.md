## Context
apply中のエージェント出力で権限要求がauto-rejectされた場合、現在は空WIPコミットの連続としてstall検出に吸収されるため、原因が権限設定にあることが明示されない。実行不能な原因を早期に判定し、再試行やstall検出を避ける必要がある。

## Goals / Non-Goals
- Goals:
  - 権限auto-rejectを検出してchangeを実行不能として扱う
  - 理由に拒否パスと権限設定の案内を含める
  - 依存スキップの判定に反映する
- Non-Goals:
  - エージェント権限設定の自動変更
  - auto-reject以外の環境構成エラーの一般化

## Decisions
- Decision: apply出力（stdout/stderrのtail）から`permission requested`と`auto-rejecting`の組を検出する
  - 理由: エージェントの標準出力/エラーに現れる明確なパターンであり、外部依存がない
- Decision: 検出時は専用のエラー種別に変換し、stalled/blockedとして扱う
  - 理由: 再試行では解消されず、人手での権限設定が必要なため

## Risks / Trade-offs
- パターンに依存するため、出力が変わると検出漏れが起きる
  - Mitigation: 正規表現は最小限のキーワードを用い、テストで代表例を固定化する

## Migration Plan
- 既存挙動は維持しつつ、新規に検出された場合のみstalled/blockedへ移行

## Open Questions
- なし
