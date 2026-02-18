## 背景
現在の `cflx` はカレントディレクトリのリポジトリを前提に動作する。サーバ常駐で複数プロジェクトを扱い、API 経由で追加/削除/同期/実行制御できるモードが必要。

## 目標 / 非目標
- 目標:
  - `cflx server` でカレントディレクトリに依存しないサーバを起動する
  - `remote_url + branch` をプロジェクト単位として管理し、永続化する
  - API v1 でプロジェクト管理と実行制御を提供する
  - ループバック以外の bind 時は bearer token 認証を必須化する
  - `~/.wt/setup` を参照/実行しない
- 非目標:
  - 既存の単一プロジェクト Web 監視（`--web`）の廃止
  - 外部 Git ホスティング固有の認証/課金運用を仕様化する
  - リモート実行を無条件に許可する

## 決定事項
- Decision: サーバモードはグローバル設定のみを読み、`.cflx.jsonc` は無視する
  - Rationale: サーバはカレントディレクトリに依存しないため、設定の一貫性を優先する
- Decision: `bind` がループバック以外の場合は `auth.mode=bearer_token` を必須にする
  - Rationale: 認証なし公開を防止する
- Decision: `project_id` は `remote_url` と `branch` から決定的に生成する（`md5(remote_url + "\n" + branch)` の先頭 16 文字）
  - Rationale: 再起動後も同一 ID を維持し、衝突リスクを低減する
- Decision: Git 操作（pull/push/resolve）と実行制御はプロジェクト単位で排他する
  - Rationale: 同一リポジトリへの同時操作を避ける
- Decision: API は `api/v1` を付与する
  - Rationale: 互換性維持と将来的拡張のため
- Decision: `server.port` の既定値は `9876` とする
  - Rationale: 自動割当では接続先が不定になるため、固定既定値を用意する
- Decision: `server.data_dir` の既定値は `${XDG_DATA_HOME}/cflx/server`（未設定時は `~/.local/share/cflx/server`）とする
  - Rationale: 永続データの配置を OS ルールに合わせる

## リスク / トレードオフ
- API 権限を広げるほどリスクが増える → 認証必須化と監査ログで緩和
- プロジェクト同時実行が増えるとリソースが逼迫する → `max_concurrent_total` による上限制御

## 移行計画
1. `cflx server` と設定セクションを追加
2. レジストリ/永続化/排他ロックを実装
3. API v1 を追加

## 未解決事項
- なし
