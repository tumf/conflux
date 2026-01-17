## Context
並列モードのresolve処理はbase branch側のマージ/検証に影響します。UI上でMキー操作が同時に発火すると、base branchの衝突や状態不整合を引き起こす可能性があるため、明示的にガードする必要があります。

## Goals / Non-Goals
- Goals: resolve実行中はMキー操作を無効化し、ユーザーに理由を示す
- Non-Goals: resolve以外の操作制限や新しいキー割り当ての追加

## Decisions
- Decision: AppStateでresolve実行中を集約管理し、Changes/Worktrees両ビューのMキー操作と表示を抑止する
- Alternatives considered: Worktreesのみ無効化（ChangesのMはresolve用途だが、同じキー操作の混乱を避けるため両方抑止）

## Risks / Trade-offs
- resolve中にChanges viewからの単発resolve操作も抑止されるため、同一セッションで連続解決が必要な場合は完了待ちになる

## Migration Plan
- 既存UIに追加の状態フラグを導入し、イベント処理に合わせて更新する

## Open Questions
- resolve中の警告メッセージの文言を最終実装時に調整する
