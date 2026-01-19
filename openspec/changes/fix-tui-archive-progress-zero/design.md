## Context
TUIのアーカイブ処理中にtasks.mdがworktree上で移動された直後、進捗の再取得が0/0になり表示がリセットされる。

## Goals / Non-Goals
- Goals: アーカイブ移動直後でも進捗表示が維持されること。
- Non-Goals: 進捗計算ロジックやタスク形式の変更。

## Decisions
- Decision: TUIの進捗再取得はworktreeのアーカイブ先tasks.mdを優先し、0/0時は既存値を保持する。
- Alternatives considered: 進捗再取得を遅延させる（アーカイブ完了後のみ更新）→表示の鮮度が落ちるため不採用。

## Risks / Trade-offs
- worktree参照が失敗する場合は既存進捗を保持するため、進捗の更新が遅延する可能性がある。

## Migration Plan
- 既存のイベントハンドラとフォールバック処理を調整し、テストで確認する。

## Open Questions
- なし
