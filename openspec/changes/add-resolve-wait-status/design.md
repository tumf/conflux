## Context
並列実行ではresolveがグローバルロックでシリアライズされます。複数のchangeがarchive済みでresolve待ちになると、TUIの自動更新が`Archived`を`NotQueued`に戻してしまい、待機状態が可視化できません。

## Goals / Non-Goals
- Goals:
  - resolve待機中のchangeを明示できる状態を追加する
  - resolve待機中はSpace/@によるキュー操作を無効化する
  - 自動更新で`ResolveWait`を維持し、誤って`NotQueued`へ戻さない
- Non-Goals:
  - resolveのシリアライズ方式やロック機構の変更
  - 既存のMergeWaitフローの変更

## Decisions
- `QueueStatus::ResolveWait`を追加し、表示文字列は`resolve wait`とする
- resolveが実行中で待機しているchangeに対して`ResolveWait`を通知するイベント経路を用意する
- 自動更新ロジックで`ResolveWait`を保持し、`NotQueued`へのリセット対象から除外する

## Alternatives Considered
- `Archived`のまま待機させる: 待機が可視化できず、誤操作が続くため不採用
- `MergeWait`を流用する: 意味が異なるため不採用

## Risks / Trade-offs
- 新しいステータス追加により表示/色/テストの更新範囲が増える
- Web/TUIの状態表現を揃えないと表示差分が起きるため、共通モデル更新が必要

## Migration Plan
1. `QueueStatus`へ`ResolveWait`を追加し表示/色を定義
2. resolve待機時に`ResolveWait`を通知する経路を追加
3. 自動更新・操作制御・表示の各ロジックを更新
4. 既存テストに加えて待機状態のテストを追加
