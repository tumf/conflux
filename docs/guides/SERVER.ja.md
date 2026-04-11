# サーバーモードガイド

`cflx server` を使った常駐運用、リモート TUI、Web UI、REST API、バックグラウンドサービス管理をまとめたガイドです。

## いつサーバーモードを使うか

サーバーモードは、常駐デーモン、複数プロジェクト管理、リモート API、またはサーバー管理の提案セッションが必要なときに使います。

```bash
cflx server
```

サーバーモードでは、接続クライアント向けに Web UI と API を公開します。TUI は `--server` でリモートサーバーへ接続できます。

## Web UI とリモート監視

- **通常モード** では、`cflx` または `cflx run` に `--web` を付けてダッシュボードを有効化
- **サーバーモード** では、ダッシュボードはデーモン構成の一部
- 通常モードではローカル実行の監視用途、サーバーモードでは接続クライアント向けの監視用途として使います

```bash
# ローカル TUI + Web UI
cflx --web

# ローカルのヘッドレス実行 + Web UI
cflx run --web

# サーバーへ接続するリモート TUI
cflx tui --server http://host:39876
```

## サーバー専用設定

### 提案セッションの OPENCODE_CONFIG

この設定は `cflx server` が作成する提案セッションに適用されます。
サーバー側の提案セッションでは `OPENCODE_CONFIG` は自動生成 / 自動注入されません。
`proposal_session.transport_env.OPENCODE_CONFIG` が未設定の場合、opencode は内蔵のデフォルト設定を使います。

独自設定にしたい場合は、`OPENCODE_CONFIG` を明示的に指定してください:

```jsonc
{
  "proposal_session": {
    "transport_env": {
      "OPENCODE_CONFIG": "/absolute/path/to/opencode.json"
    }
  }
}
```

`OPENCODE_CONFIG` は任意で、カスタム opencode 設定ファイルを使いたいときだけ必要です。

## Web UI とダッシュボード

Web UI は HTTP / WebSocket ベースの監視ダッシュボードです。通常モード（`--web`）とサーバーモードの両方で利用できます。

### Web UI を有効化する

```bash
# 通常モード: TUI + Web UI
cflx --web

# 通常モード: ヘッドレス実行 + Web UI
cflx run --web

# カスタムポートと bind アドレス
cflx --web --web-port 9000 --web-bind 0.0.0.0
```

デフォルトポート（0）を使うと、OS が利用可能なポートを自動割当します。
実際に bind されたアドレスはサーバー起動時にログへ出力されます。

サーバーモード（`cflx server`）では、Web UI は設定ポートで常に利用できます。

### ダッシュボード機能

- **ダッシュボード UI**: `http://localhost:<port>/` で進捗を確認
- **リアルタイム更新**: WebSocket 接続で進捗をライブ更新
- **REST API**: 状態をプログラムから取得可能
- **QR コードポップアップ**: TUI で `w` を押すとモバイル向けの QR コードを表示

### REST API エンドポイント

| エンドポイント | メソッド | 説明 |
|----------|--------|-------------|
| `/api/health` | GET | ヘルスチェック |
| `/api/state` | GET | オーケストレーター全状態 |
| `/api/changes` | GET | 進捗付きの変更一覧 |
| `/api/changes/{id}` | GET | 特定変更の詳細 |

完全な API 仕様は [../openapi.yaml](../openapi.yaml) を参照してください。

### WebSocket

リアルタイム状態更新のために `ws://localhost:<port>/ws` へ接続します。メッセージは以下形式の JSON です:

```json
{
  "type": "state_update",
  "timestamp": "2024-01-12T10:30:00Z",
  "changes": [
    {
      "id": "add-feature",
      "completed_tasks": 3,
      "total_tasks": 10,
      "progress_percent": 30.0,
      "status": "in_progress"
    }
  ]
}
```

### ダッシュボード概要

```
┌─────────────────────────────────────────────────────────────────┐
│  OpenSpec Orchestrator                           ● Connected    │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐            │
│  │    5    │  │    2    │  │    1    │  │    2    │            │
│  │  Total  │  │Complete │  │Progress │  │ Pending │            │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ add-feature-auth                    [IN_PROGRESS]           │
│  │ ████████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  40%    │   │
│  │ 4/10 tasks                                               │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ fix-login-bug                       [COMPLETE]              │
│  │ ████████████████████████████████████████████████  100%  │   │
│  │ 5/5 tasks                                                │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ refactor-api                        [PENDING]               │
│  │ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  0%    │   │
│  │ 0/8 tasks                                                │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│  Last updated: 2024-01-12 10:30:00                              │
└─────────────────────────────────────────────────────────────────┘
```

- **統計バー**: 総数、完了、進行中、保留中の変更数を表示
- **変更カード**: 各変更に ID、進捗状態、プログレスバーを表示
- **リアルタイム更新**: WebSocket 接続経由で自動更新
- **接続状態表示**: 現在の WebSocket 接続状態（Connected / Disconnected）を表示
- **レスポンシブデザイン**: デスクトップ / モバイル両対応

### Web UI のトラブルシューティング

| 問題 | 解決策 |
|-------|----------|
| "Address already in use" | `--web-port 0`（デフォルト）で OS に自動割当させるか、未使用ポートを指定 |
| ダッシュボードが開かない | `--web` が有効か確認し、URL に正しいポートが含まれているか確認 |
| WebSocket が頻繁に切れる | ネットワークの安定性を確認。ダッシュボードは自動再接続します |
| 変更が更新されない | ページを再読込するか、オーケストレーターが実際に処理中か確認 |
| 別デバイスからアクセスできない | 外部接続を許可するには `--web-bind 0.0.0.0` を利用（ローカルネットワーク向け） |
| ブラウザコンソールで CORS エラー | クロスオリジン要求では通常の挙動です。サーバー側で CORS ヘッダーを処理します |

## バックグラウンドサービス (`cflx service`)

`cflx service` を使うと、`cflx server` をユーザーレベルのバックグラウンドサービスとして導入・管理できます。

- macOS: `launchd` ユーザーエージェント
- Linux: `systemd --user` サービス
- Windows: タスクスケジューラ

```
cflx service <install|uninstall|status|start|stop|restart>
```

例:

```bash
# サービスをインストールして有効化
cflx service install

# バックグラウンドサーバーを開始または再起動
cflx service start
cflx service restart

# サービス状態を確認
cflx service status

# 停止または削除
cflx service stop
cflx service uninstall
```

補足:

- `install`、`start`、`restart` は、サービスマネージャを触る前に有効なグローバル `server` 設定を検証します。
- macOS では `~/Library/LaunchAgents/com.conflux.cflx-server.plist` に plist を書き込みます。
- Linux では `~/.config/systemd/user/cflx-server.service` に unit file を書き込みます。
- 永続的なサーバー設定は、インストール前に `~/.config/cflx/config.jsonc` などのグローバル設定ファイルで行ってください。
