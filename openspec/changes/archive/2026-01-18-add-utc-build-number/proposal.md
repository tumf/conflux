# Change: UTC時刻ベースのビルド番号導入と表示

## Why
ビルド単位で追跡できる識別子がなく、配布物やログの突合が難しいため、UTC時刻ベースのビルド番号を導入します。

## What Changes
- CLI/TUIのバージョン表示にUTC時刻ベースのビルド番号を追加
- 表示形式を `v<semver>(YYYYMMDDHHmmss)` に統一

## Impact
- Affected specs: `specs/cli/spec.md`
- Affected code: `src/cli.rs`, `src/tui/utils.rs` などバージョン表示箇所
