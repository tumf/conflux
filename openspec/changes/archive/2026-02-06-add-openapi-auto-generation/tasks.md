## 1. Implementation
- [x] 1.1 OpenAPI 生成クレートの選定と依存追加（例: utoipa）
- [x] 1.2 Web API のパス/スキーマ定義をコードに追加
- [x] 1.3 生成コマンドを追加し `docs/openapi.yaml` を生成する
- [x] 1.4 `make openapi` と `make check-openapi` を追加する
- [x] 1.5 CI で `make check-openapi` を実行して差分を検知する
- [x] 1.6 README/README.ja の OpenAPI 参照パスを更新する

## Acceptance #1 Failure Follow-up
- [x] OpenAPI 仕様に WebSocket `/ws` を含める（`src/bin/openapi_gen.rs` と `docs/openapi.yaml` に反映されること）
