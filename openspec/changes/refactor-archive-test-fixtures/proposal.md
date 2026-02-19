# Change: Archive test fixture helpers

## Why
`src/execution/archive.rs` のテストで TempDir 作成やディレクトリ構築が繰り返されており、意図の読み取りと変更時のメンテが難しくなっています。共通フィクスチャ化で重複を減らし、保守性を高めます。

## What Changes
- アーカイブ関連テストの共通セットアップをヘルパー関数に集約する
- テストの意図を保ったまま重複コードを削減する
- 既存の挙動・検証内容は変更しない

## Impact
- Affected specs: code-maintenance
- Affected code: `src/execution/archive.rs`

## Acceptance Criteria
- `cargo test` が成功する
- テストの期待値・検証観点が変わらない
- 公開 API とアーカイブ挙動に変更がない
