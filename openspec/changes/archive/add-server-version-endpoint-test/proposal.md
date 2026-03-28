# Change: Version エンドポイントの統合テスト追加

**Change Type**: implementation

## Why

`GET /api/v1/version` エンドポイントにテストが存在せず、回帰検出ができない。今回のセッションで古いバイナリが 404 を返していたインシデントでも、テストがあればリリース前に検出できた。

## What Changes

- `src/server/api.rs` の `#[cfg(test)]` に version エンドポイントのユニットテストを追加
- 認証なし／認証ありの両方のシナリオをカバー

## Impact

- Affected specs: server-mode
- Affected code: `src/server/api.rs` (テストのみ)
