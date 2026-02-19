# Change: Remote module test helpers

## Why
`src/remote/ws.rs` と `src/remote/client.rs` のテストでモックサーバー生成やタイムアウト待機が重複しており、保守と可読性が低下しています。共有ヘルパー化で意図を明確にします。

## What Changes
- WS/HTTP のモックサーバー生成を共通ヘルパーに集約する
- JSON フィクスチャ/検証補助を共有化する
- 既存のテスト意図と挙動は維持する

## Impact
- Affected specs: code-maintenance
- Affected code: `src/remote/ws.rs`, `src/remote/client.rs`, `src/remote/types.rs`

## Acceptance Criteria
- `cargo test` が成功する
- リモート関連テストの検証観点が変わらない
- 本番コードの挙動に変更がない
