## Implementation Tasks

- [x] 1. `src/server/api.rs` に `GET /version` ハンドラを追加する。レスポンス: `{ "version": "v0.5.24 (build)" }` 形式。`public_api_routes` に配置して認証不要とする (verification: `cargo build` が通り、`GET /api/v1/version` で 200 + JSON が返ること)
- [x] 2. `dashboard/src/api/restClient.ts` に `fetchVersion(): Promise<{ version: string }>` を追加する (verification: TypeScript コンパイルが通ること)
- [x] 3. `dashboard/src/components/Header.tsx` を修正し、マウント時に `fetchVersion()` を呼び出してバージョン文字列を取得・表示する。ロゴ名「Conflux」の右隣に `text-xs text-[#52525b]` で表示。取得失敗時は非表示 (verification: `cd dashboard && npm run build` が通ること)
- [x] 4. `cargo clippy -- -D warnings` と `cargo fmt --check` が通ることを確認する (verification: lint エラーなし)
