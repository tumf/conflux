## Implementation Tasks

- [x] `src/server/api.rs` の `#[cfg(test)]` モジュールに `test_get_version_returns_200` テストを追加 (verification: `cargo test test_get_version_returns_200`)
- [x] `test_get_version_no_auth_required` テストを追加: bearer token が設定されていても認証なしで 200 を返すことを確認 (verification: `cargo test test_get_version_no_auth_required`)
- [x] `test_get_version_response_format` テストを追加: レスポンスに `version` フィールドが含まれることを確認 (verification: `cargo test test_get_version_response_format`)

## Future Work

- なし
