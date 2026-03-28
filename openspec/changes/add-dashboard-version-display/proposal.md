# Change: ダッシュボードHeaderにConfluxバックエンドのバージョンを表示

**Change Type**: implementation

## Why

ダッシュボードにバージョン情報がなく、どのビルドが動作しているか判別できない。TUI側には既にヘッダーにバージョンが表示されている。

## What Changes

- Server API (`/api/v1`) に `GET /version` エンドポイントを追加し、バックエンドのバージョン文字列を返す
- ダッシュボードの `Header.tsx` にバージョンを小さく表示する（ロゴ名の右隣に `text-xs` で控えめに表示）

## Impact

- Affected specs: `web-monitoring`（バージョンエンドポイントとダッシュボード表示要件を追加）
- Affected code: `src/server/api.rs`, `dashboard/src/api/restClient.ts`, `dashboard/src/components/Header.tsx`
