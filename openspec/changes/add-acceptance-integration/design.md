## Context
acceptance の処理は実装されているが、オーケストレーションの実行経路に組み込まれていないため、apply と archive の間で acceptance を必ず実行するという仕様が満たされていない。

## Goals / Non-Goals
- Goals: 逐次/並列の両方で apply 成功後に acceptance を必ず実行し、結果に応じてループを分岐させる
- Goals: acceptance の結果を履歴・ログ・状態遷移として記録し、次回 apply/acceptance の文脈に反映できるようにする
- Non-Goals: acceptance コマンドや出力フォーマットの仕様変更

## Decisions
- Decision: 逐次実行は `src/orchestrator.rs` の apply 成功後に acceptance を挿入し、PASS のみ archive に進める
- Decision: 並列実行は parallel executor の apply 完了後に acceptance を挿入し、PASS のみ archive に進める
- Decision: acceptance 成功時に該当 change の acceptance 履歴をクリアする

## Risks / Trade-offs
- acceptance が失敗し続ける場合、apply 反復回数が増えて処理時間が増加する
- 並列実行で acceptance が長時間になる場合、同時実行スロットが占有される

## Migration Plan
- 既存の apply/archvie フローに acceptance を追加し、既存の config と互換性を維持する

## Open Questions
- なし
