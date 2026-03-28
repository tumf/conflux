# Change: Run/Stop をオーケストレーション全体制御に変更

## Problem/Context

現在の Run/Stop はプロジェクト単位 (`POST /projects/{id}/control/run|stop|retry`) で設計されている。ダッシュボードの `ProjectCard` にも per-project の Run/Stop ボタンがある。

実際の運用では、Conflux サーバーは全プロジェクトを一括でオーケストレーションとして制御する方が自然であり、実行対象の粒度は個別 change の選択マーク (`selected`) で制御する（前提: `add-server-change-selection`）。

## Proposed Solution

1. per-project の `control/run`, `control/stop`, `control/retry` エンドポイントを廃止
2. グローバルの `POST /api/v1/control/run` と `POST /api/v1/control/stop` を新設
3. サーバーにグローバル `OrchestrationStatus` (Idle/Running/Stopped) を追加
4. グローバル Run 時: 全プロジェクトの `selected: true` な change を対象に runner を spawn
5. グローバル Stop 時: 全 running プロジェクトを graceful stop
6. ダッシュボードの Header にグローバル Run/Stop ボタンを配置
7. `ProjectCard` から Run/Stop/Retry ボタンを削除（Sync/Delete は残す）
8. Running 状態中に新規プロジェクトが追加されたら自動的に runner を spawn

## Acceptance Criteria

- `POST /api/v1/control/run` で全プロジェクトの selected change のオーケストレーションが開始される
- `POST /api/v1/control/stop` で全 running プロジェクトが graceful stop される
- per-project の `control/run`, `control/stop`, `control/retry` エンドポイントが存在しない
- WebSocket `full_state` に `orchestration_status` フィールドが含まれる
- ダッシュボード Header にグローバル Run/Stop ボタンが表示される
- `ProjectCard` に Run/Stop/Retry ボタンが存在しない
- Running 中にプロジェクトを追加すると自動的にオーケストレーションに参加する
- `max_concurrent_total` セマフォは引き続き有効

## Out of Scope

- TUI モードの変更（TUI は従来通りローカル単一プロジェクト操作）
- `cflx run` CLI の変更（単体実行は変更なし）
