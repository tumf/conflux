# Change: Server mode 仕様に version エンドポイント要件を追加

**Change Type**: spec-only

## Why

`GET /api/v1/version` エンドポイントは実装済みだが、server-mode 仕様に要件が記載されていない。仕様とコードの乖離を解消し、version エンドポイントの振る舞いを正規化する。

## What Changes

- server-mode 仕様に version エンドポイントの要件とシナリオを追加

## Impact

- Affected specs: server-mode
- Affected code: なし（spec-only）
