## Context
並列実行ではresolveがグローバルロックでシリアライズされます。複数のchangeがarchive済みでresolve待ちになると、TUIの自動更新が`Archived`を`NotQueued`に戻してしまい、待機状態が可視化できません。

`update-workspace-archive-detection`で実装された`WorkspaceState::Archived`判定を活用し、worktree内でarchive済みだがmerge未完了のchangeを冪等に識別できます。

## Goals / Non-Goals
- Goals:
  - resolve待機中のchangeを明示できる`ResolveWait`状態を追加する
  - `WorkspaceState::Archived`を活用し、worktree内でarchive済みのchangeを識別する
  - resolve待機中はSpace/@によるキュー操作を無効化する
  - 自動更新で`ResolveWait`を維持し、誤って`NotQueued`へ戻さない
- Non-Goals:
  - resolveのシリアライズ方式やロック機構の変更
  - 既存のMergeWaitフローの変更

## Decisions
- `QueueStatus::ResolveWait`を追加し、表示文字列は`resolve wait`とする
- TUIの自動更新/再起動時に`detect_workspace_state`で`WorkspaceState::Archived`が検出されたchangeは`ResolveWait`として識別する
- 自動更新ロジックで`ResolveWait`を保持し、`NotQueued`へのリセット対象から除外する

## Alternatives Considered
- `Archived`のまま待機させる: 待機が可視化できず、誤操作が続くため不採用
- `MergeWait`を流用する: 意味が異なるため不採用
- イベントベースで通知する: WorkspaceState判定の方が冪等であるため不採用

## Risks / Trade-offs
- 新しいステータス追加により表示/色/テストの更新範囲が増える
- Web/TUIの状態表現を揃えないと表示差分が起きるため、共通モデル更新が必要

## Migration Plan
1. `QueueStatus`へ`ResolveWait`を追加し表示/色を定義
2. TUIで`WorkspaceState::Archived`を検出し`ResolveWait`として表示する
3. 自動更新・操作制御・表示の各ロジックを更新
4. 既存テストに加えて待機状態のテストを追加
