# Change: サーバー側に change の selected 状態を追加

## Problem/Context

現在、change の選択状態 (`selected: bool`) は TUI の `ChangeState` にのみ存在し、サーバーモードには持たない。ダッシュボードからは個別 change の実行対象を制御する手段がない。

グローバル Run/Stop（後続提案 `globalize-run-stop`）の前提として、サーバー側で「どの change を実行対象にするか」を管理する必要がある。

## Proposed Solution

1. サーバーの `ProjectRegistry` に per-change の `selected: bool` 状態を保持する
2. 新規検出された change はデフォルト `selected: true`
3. REST API で toggle / toggle-all エンドポイントを追加
4. `RemoteChange` に `selected` フィールドを追加し WebSocket で配信
5. ダッシュボードの `ChangeRow` にチェックボックス UI を追加

## Acceptance Criteria

- サーバー起動時、全 change が `selected: true` で初期化される
- `POST /api/v1/projects/{id}/changes/{change_id}/toggle` で selected が反転する
- `POST /api/v1/projects/{id}/changes/toggle-all` で全 change の selected が一括トグルされる
- WebSocket `full_state` および `change_update` に `selected` フィールドが含まれる
- ダッシュボードで各 change 行にチェックボックスが表示され、クリックで toggle API が呼ばれる
- サーバー再起動時は全 change が `selected: true` にリセットされる（永続化不要）

## Out of Scope

- グローバル Run/Stop（別提案 `globalize-run-stop`）
- TUI の selected 状態との同期（TUI は独自の selected を維持）
