# Change: cflx server モード専用ダッシュボードの追加

## Why

`cflx server` モードは複数プロジェクトを管理するが、既存の Web ダッシュボード（`web/`）は
単一プロジェクトの run モード向け vanilla JS 実装であり、サーバモードには対応していない。
複数プロジェクトの実行状態・変更進捗・ログをリアルタイムで管理・操作できる専用 UI が必要。

## What Changes

- **新規**: `dashboard/` ディレクトリに React + shadcn/ui ダッシュボードを追加
  - Vite でビルドし、`dashboard/dist/` に出力
  - Rust 側でビルド済み静的ファイルを `include_str!` で埋め込み、`/dashboard/` で配信
- **新規**: `src/server/api.rs` に `/dashboard` 静的ファイル配信ルートを追加
- **対象外**: 既存 `web/` ディレクトリ（run モード用）は変更しない

## UI 仕様

### レイアウト（3 カラム）

```
┌──────────────────────────────────────────────────────────────┐
│ Conflux Server                              ● Connected      │
├──────────────┬──────────────────────────┬───────────────────-┤
│  Projects    │  Changes                 │  Logs              │
│              │  (selected project)      │  (selected project)│
│  repo@main   │                          │                    │
│  running     │  add-feature-x           │  [INFO] apply:1    │
│  ▶ ⏹ ↺ ✕  │  ████░░ 3/5 applying:2  │  [WARN] retry...   │
│              │                          │                    │
│  repo2@dev   │  refactor-api            │                    │
│  idle        │  ████████ 8/8 archived  │                    │
│  ▶ ⏹ ↺ ✕  │                          │                    │
└──────────────┴──────────────────────────┴────────────────────┘
```

### Projects パネル（左）

- プロジェクト名表示: `repo@branch` 形式
- 実行状態バッジ: `idle` / `running` / `stopped`
- アクション（Lucide アイコン）:
  - ▶ Run → `POST /api/v1/projects/{id}/control/run`
  - ⏹ Stop → `POST /api/v1/projects/{id}/control/stop`
  - ↺ Git Sync → `POST /api/v1/projects/{id}/git/sync`、toast で結果通知
  - ✕ Delete → `DELETE /api/v1/projects/{id}`、AlertDialog 確認付き
- クリックで Changes / Logs パネルをそのプロジェクトにフォーカス

### Changes パネル（中）

- 選択プロジェクトの変更一覧（`RemoteChange[]`）
- 変更ごとにプログレスバー（`completed_tasks / total_tasks`）
- ステータス表示: `idle | queued | applying | accepting | archiving | resolving | archived | merged | error`
- iteration がある場合: `applying:2` 形式

### Logs パネル（右）

- WebSocket `Log { entry: RemoteLogEntry }` をリアルタイム表示
- 選択プロジェクト（`project_id`）でフィルタ
- レベル別色分け: info=通常、warn=黄、error=赤
- 末尾オートスクロール

### WebSocket 接続管理

- 接続先: `ws://<host>/api/v1/ws`（server モードの既存 WS エンドポイント）
- `FullState` 受信で全状態を更新（2 秒間隔）
- `Ping` は無視
- 再接続: exponential backoff（1s → 2s → 4s → max 30s）
- 接続状態インジケータ: Connected / Reconnecting / Disconnected

## Impact

- Affected specs: `server-mode`（dashboard 要件を追加）、`web-monitoring`（server dashboard として新規節を追加）
- Affected code: 新規 `dashboard/`、`src/server/api.rs`（静的配信ルート追加）
- 既存 `web/`（run モード用）: **変更なし**
