## Context
apply 完了後に仕様充足の確認がなく、そのまま archive へ進む構造になっている。
apply の反復と archive の間に受け入れ検査を挿入し、失敗時は apply に戻す必要がある。

## Goals / Non-Goals
- Goals: acceptance_command による受け入れ検査を追加し、合否に応じて apply/ archive を制御する
- Goals: 受け入れ失敗時に指摘事項を apply 履歴へ注入できるようにする
- Non-Goals: 受け入れ検査の実装内容（外部ツール/プロンプト）を本体で自動生成する

## Decisions
- Decision: 設定に `acceptance_command` と `acceptance_prompt` を追加し、{change_id}/{prompt} プレースホルダー展開を行う
- Decision: 受け入れ合否は stdout テキストを解析し、exit code は「コマンド実行の成否」として扱う
- Decision: apply と archive の間に acceptance loop を挿入し、失敗時は指摘事項を apply ループへ戻す

## Risks / Trade-offs
- 出力テキストのフォーマットが崩れると誤判定のリスクがある
- acceptance の失敗が続く場合、apply 反復回数が増え実行時間が増大する

## Migration Plan
- config に新項目追加（既存設定はデフォルトで動作維持）
- apply/parallel フローに acceptance を挿入
- 既存の archive 履歴/適用履歴に統合する

## Open Questions
- acceptance 出力フォーマットの詳細仕様（合否/指摘事項のタグ形式）
