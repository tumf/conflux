## Context
- 並列 apply/archive の履歴注入が欠落しており、再試行時のプロンプトに前回情報が出ない
- archive の既定 system-context が実装と乖離しており誤誘導になる
- resolve/analysis のログヘッダーが消えており試行回数が追えない

## Goals / Non-Goals
- Goals: 履歴注入の完全化、archive 既定プロンプトの無害化、ログヘッダーの復旧
- Non-Goals: 新しいUIやログフォーマットの大幅な刷新

## Decisions
- archive_prompt の既定値は空文字にする（system-context を使わない）
- apply/archive は逐次/並列とも同一の履歴注入ロジックに統一する
- resolve/analysis はログヘッダーに試行番号を必ず含める

## Risks / Trade-offs
- 履歴注入が増えるためプロンプトが長くなる

## Migration Plan
- 既存の履歴フォーマットを維持しつつ注入箇所を統一する

## Open Questions
- なし
