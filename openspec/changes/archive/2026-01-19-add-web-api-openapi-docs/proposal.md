# Change: APIエンドポイントのOpenAPI YAMLをdocsに追加

## Why
Web UI 用の REST API / WebSocket の仕様が実装と spec に分散しており、クライアント実装や外部連携で参照できる OpenAPI 形式のマニュアルが不足しているため。

## What Changes
- `docs/` 配下に OpenAPI 3.1 形式の YAML マニュアルを追加する
- Web 監視 API のエンドポイントとレスポンス構造を OpenAPI で明文化する
- WebSocket 接続 (`/ws`) の仕様は OpenAPI の拡張フィールドで補足する

## Impact
- Affected specs: documentation, web-monitoring
- Affected code: none (documentation only)
