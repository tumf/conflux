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

## Web UI: オペレーターコンソール

Web UI は静的 3 ファイル（`/`, `/style.css`, `/app.js`）として配信される組み込みオペレーターコンソールです。唯一の通信先は `/api/v2` で、読み取り・イベント・エラー・変更操作のすべてがバージョン付きリモートコントロール契約を通ります。したがってブラウザにも、他のコントローラーと同じ Bearer 認証・楽観的リビジョン・冪等性・型付きエラーが適用されます。通常モード（`--web`）とサーバーモードの両方で利用できます。

### Web UI を有効化する

```bash
# 通常モード: TUI + Web UI
cflx --web

# 通常モード: ヘッドレス実行 + Web UI
cflx run --web

# カスタムポート。ループバック以外の bind にはベアラートークンが必要
export CFLX_WEB_TOKEN="$(openssl rand -hex 32)"
cflx --web --web-port 9000 --web-bind 0.0.0.0 --web-auth-token-env CFLX_WEB_TOKEN
```

デフォルトポート（0）を使うと、OS が利用可能なポートを自動割当します。
実際に bind されたアドレスはサーバー起動時にログへ出力されます。

ループバック以外のアドレスに bind する場合、`/api/v2` リモート制御 API 用の
ベアラートークンが必須で、未指定ならプロセスは起動を拒否します。
`--web-auth-token-env VAR`（推奨）または `--web-auth-token TOKEN` のどちらか一方を
指定してください（同時指定は不可）。詳細は
[USAGE.md](USAGE.md#remote-control-api-apiv2) を参照してください。

サーバーモード（`cflx server`）では、Web UI は設定ポートで常に利用できます。

### コンソールが表示するもの

- **現在の状態を最優先**: プロセス識別子、接続の鮮度、アプリケーションモード、実行中の作業、対応が必要な項目、そして次に取れる唯一の有効なアクション。
- **変更の優先度別グルーピング**: 「対応が必要」「実行中」「待機中」「完了」。各行には詳細を開くラベル付きボタンがあります。
- **ワークツリー**: 不透明な `worktree_id` のみで指定し、操作可否はサーバーの判定をそのまま表示します。拒否された場合はサーバーが返した理由を表示します。
- **ログとエラー**: レベルフィルタ付きの常設ログビュー。型付き API エラーは `error_code`・相関 ID・次の対処とともに画面に残ります。
- **QR コードポップアップ**: TUI で `w` を押すとモバイル向けの QR コードを表示。

### ブラウザ認証

トークンが必要なインスタンスでは、最初の 401 応答を受けた時点でラベル付きトークンフォームを表示します。トークンは `Authorization` ヘッダーにのみ送られ、URL・ログ・相関 ID・`localStorage` には一切現れません。リロードを跨ぐためにタブ単位の `sessionStorage` に保持しますが、**Disconnect** で保護対象の値ごと消去されます。

### イベントストリームと復旧

コンソールは `fetch()` のレスポンスストリーミングで `/api/v2/events` を読み、`instance_id` と `event_sequence` を追跡します。リプレイギャップ、シーケンスの不連続、解釈できないフレーム、プロセス世代の変化が起きた場合は、ライブ観測を再開する前に `/api/v2/state` を読み直します。ストリーミングが利用できない場合は no-store のスナップショットポーリングにフォールバックします。表示中の状態が最新でないときは *reconnecting* / *stale* / *disconnected* を明示し、信頼できる状態に戻るまでコマンド送信を拒否します。

### コマンド

すべての変更操作は `POST /api/v2/commands` で、直近に確認した `state_revision` と意図ごとの冪等キーを伴います。実行待ちの間に同じ操作を再度押しても何も起きません。再送でキーを再利用するのは、通信結果が不明なときだけです。`stale_revision` が返った場合は状態を更新し、副作用を再実行せずに再判断を求めます。強制停止・実行中変更の停止・ワークツリー削除は、リクエスト送信前にアクセシブルな確認ダイアログを必須とします。

### API

HTTP 契約は `/api/v2` のみです。コンソールの移行完了に伴い、旧来のバージョンなし `/api/*` ルートとブラウザ向け `/ws` は削除されました。これらへのリクエストは 404 を返し、副作用はありません。エンドポイント一覧は [USAGE.md](USAGE.md#remote-control-api-apiv2)、生成済みスキーマは [../openapi.yaml](../openapi.yaml) を参照してください。

### Web UI のトラブルシューティング

| 問題 | 解決策 |
|-------|----------|
| "Address already in use" | `--web-port 0`（デフォルト）で OS に自動割当させるか、未使用ポートを指定 |
| コンソールが開かない | `--web` が有効か確認し、URL に正しいポートが含まれているか確認 |
| 「Authentication required」から進まない | インスタンスにトークンが設定されています。トークンフォームに貼り付けてください |
| 接続表示が Stale / Disconnected | 意図的に操作が無効化されています。**Refresh now** を押すか、プロセスが稼働中か確認してください |
| `stale_revision` で操作が拒否される | 状態が更新されています。最新状態を確認して選び直してください |
| 別デバイスからアクセスできない | 外部接続を許可するには `--web-bind 0.0.0.0` を利用（ローカルネットワーク向け。`--web-auth-token-env` も必須） |
| ブラウザコンソールで CORS エラー | `/api/v2` は既定で同一オリジンのみ許可します。逆プロキシで外部オリジンが変わる場合は `--web-allowed-origin` で明示してください |

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
