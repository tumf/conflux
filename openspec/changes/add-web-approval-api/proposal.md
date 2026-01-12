# Change: Web UIからの変更承認(approve)機能

## Why
現在、変更の承認操作（TUIの`@`キー）はTUI経由でしか実行できない。Webダッシュボードからも同様の承認操作を可能にすることで、モバイルデバイスやリモート環境からの承認ワークフローを実現する。

## What Changes
- REST APIに変更承認用のエンドポイントを追加
  - `POST /api/changes/{id}/approve` - 変更を承認
  - `POST /api/changes/{id}/unapprove` - 変更の承認を解除
- Webダッシュボードに承認ボタンを追加
- 承認状態変更時のWebSocket通知を実装

## Impact
- Affected specs: `web-monitoring`
- Affected code:
  - `src/web/api.rs` - REST APIハンドラー追加
  - `src/web/mod.rs` - ルーティング追加
  - `web/app.js` - UIコンポーネント追加
  - `web/style.css` - ボタンスタイル追加
