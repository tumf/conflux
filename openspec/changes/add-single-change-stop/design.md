## Context
現在のTUIは実行中changeに対してSpace/@操作を拒否しており、停止は全体停止のみです。単体停止を追加するには、UI操作、実行エンジン（serial/parallel）、状態遷移とイベント連携を変更する必要があります。

## Goals / Non-Goals
- Goals:
  - 実行中changeを1件だけ停止できる
  - 停止後は`not queued`に戻り、実行マークが外れる
  - 他のqueuedは継続して処理される
- Non-Goals:
  - 追加の新ステータス（例: stopping）導入
  - Web APIからの単体停止操作（今回はTUIのみ）

## Decisions
- Decision: Spaceキーはactive changeに対して「単体停止要求」として扱う
  - Rationale: 既存のキーバインドに合わせ、操作を増やさず導入できるため
- Decision: 停止はイベント完了時に`not queued`へ遷移し、即時の状態変更は行わない
  - Rationale: キャンセル失敗時にUI状態が不整合にならないようにするため
- Decision: Serial/Parallelともにchange単位のキャンセル経路を追加する
  - Rationale: 全体停止とは独立した粒度を提供し、他のqueued継続を保証するため

## Alternatives Considered
- A) 新しい`QueueStatus::Stopping`を追加して即時フィードバックを出す
  - Pros: 見た目が分かりやすい
  - Cons: ステータス語彙の拡張とWeb/TUI両方への影響が大きい
- B) active changeへのSpaceは無効のまま、別キーで停止
  - Pros: 既存動作の変更が少ない
  - Cons: 操作が増え、学習コストが高い

## Risks / Trade-offs
- キャンセル完了までの時間差でユーザーが停止結果を把握しづらい
  - Mitigation: ログに停止要求/停止完了/停止失敗を明示する

## Migration Plan
1) TUIコマンドとイベントの追加
2) Serial/Parallelで単体キャンセルの経路を実装
3) UI状態遷移とキーヒントの更新

## Open Questions
- なし
