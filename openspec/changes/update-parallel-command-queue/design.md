## Context

parallel 実行の apply/archive は `CommandQueue` を経由しておらず、stagger/retry が無効になっている。command-queue 仕様では並列 apply/archive に対しても同一のキュー設定を適用することが求められているため、parallel executor の起動経路を統一する必要がある。

## Goals / Non-Goals

- Goals:
  - parallel apply/archive を CommandQueue 経由に統一する
  - shared stagger state を worktree 間で共有する
  - streaming 出力とリトライ通知の挙動を維持する

- Non-Goals:
  - resolve/analyze の実行経路統合
  - retry ルールや設定値の変更
  - 挙動変更（ユーザー体験、イベント順序、ログ文言の変更）

## Decisions

- Decision: parallel executor に共有 CommandQueue を持たせ、apply/archive のみその経路を通す
- Decision: stagger state は parallel executor のライフサイクルで共有する

## Risks / Trade-offs

- stagger により並列の起動が直列化されるため、瞬間的なスループットが低下する
- リトライ通知が増えるため、TUI ログがノイジーになる可能性がある

## Migration Plan

1. parallel executor に CommandQueue と shared stagger state を導入する
2. apply/archive を CommandQueue 経由に切り替える
3. 既存の ParallelEvent 出力順序を維持できていることを確認する

## Open Questions

- リトライ通知の文言を TUI のログ文言ポリシーに合わせる必要があるか？
