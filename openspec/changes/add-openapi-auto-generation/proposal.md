# Change: OpenAPI仕様の自動生成と更新必須化

## Why
Web API の OpenAPI YAML が手動更新で漂流しやすく、実装との不整合が発生しているため。

## What Changes
- Web 監視 API の OpenAPI 3.1 YAML をコードから自動生成する
- 生成物の配置先を `docs/openapi.yaml` に統一する
- `make openapi` と `make check-openapi` により生成と差分検知を行う
- CI で差分がある場合は失敗させ、更新を必須化する

## Impact
- Affected specs: documentation, web-monitoring
- Affected code: `Cargo.toml`, `src/web/`, `Makefile`, CI workflow, `README.md`, `README.ja.md`
