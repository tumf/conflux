## Context

parallel 実行の apply/archive は現状 `CommandQueue` を通らず直接 `sh -c` で実行されるため、stagger/retry が効かず同時起動が発生する。serial 側は `CommandQueue` を経由しており、挙動の差異がある。

## Goals / Non-Goals

- Goals:
  - parallel 実行でも apply/archive を CommandQueue 経由に統一する
  - stagger/retry の既存設定を parallel 側で適用する
  - streaming 出力を保持し、TUI/CLI の出力体験を維持する

- Non-Goals:
  - resolve コマンドの実行方式変更
  - opencode 側のキャッシュ設計やロック機構の変更

## Decisions

- Decision: parallel 用の `CommandQueue` を共有し、apply/archive をそのキューで実行する
- Reasoning: 既存の `command_queue_*` 設定をそのまま活用でき、serial と同等の retry/stagger を提供できる
- Alternatives considered:
  - 各 workspace ごとに独立キューを持つ（stagger が効かないため却下）
  - 外部ロックファイルで直列化する（設定が分散し運用が複雑になるため却下）

## Risks / Trade-offs

- 既存の parallel throughput は低下する可能性がある（stagger により起動が直列化される）
- retry 出力が多くなるため、TUI ログのノイズが増える可能性がある

## Migration Plan

1. parallel executor に共有 CommandQueue を導入する
2. apply/archive の実行経路を CommandQueue 経由に変更する
3. 既存の event/出力が維持されていることを確認する

## Open Questions

- リトライ通知の文言を既存の TUI ログに合わせて統一すべきか？
