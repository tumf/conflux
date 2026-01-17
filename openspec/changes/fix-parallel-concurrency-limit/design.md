## Context
parallel modeの実装では、group内の全changeに対してworktreeを先に作成してからapply/archiveを並列実行している。このため、semaphoreによる同時数制御がworktree作成フェーズに適用されず、結果として上限を超えるworktreeが同時に作成・実行される。

## Goals / Non-Goals
- Goals:
  - worktree作成・apply・archiveの全工程で同時実行数上限を守る
  - TUI/CLIで上限の挙動を一致させる
- Non-Goals:
  - 並列化アルゴリズムの再設計
  - 依存関係解析やキューの仕様変更

## Decisions
- Decision: worktree作成をsemaphore制御下に移動し、1 changeごとに「create → apply → archive → merge → cleanup」を完結させる。
- Alternatives considered: group開始時点で上限分だけworktreeを作成し、残りは後続で作成する方式。
- Rationale: 作成/実行の同時数を単一の制御に統合でき、TUI/CLIの挙動差異が最小になる。

## Risks / Trade-offs
- 1 change単位でworktree作成を行うため、groupの開始ログから実際の作成完了までの時間が相対的に長く感じる可能性がある。
- merge/cleanupのタイミングが現在と同じであることを確認する必要がある。

## Migration Plan
- 既存のフローを段階的に置換し、worktree一括作成を削除する。
- 既存の並列実行ログやイベントに影響が出ないか確認する。

## Open Questions
- なし
