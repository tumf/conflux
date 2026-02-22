# 変更提案: cflx project sync --all の追加

## Why（背景）
登録済みプロジェクトを一括で同期したいニーズがあり、現状は個別に `cflx project sync <project_id>` を繰り返す必要がある。`--all` オプションで全件同期できるようにし、運用負荷を下げる。

## What Changes（変更内容）
- `cflx project sync --all` を追加し、登録済みプロジェクトをすべて同期する。
- 同期は個別結果を表示し、失敗が含まれる場合は非 0 で終了する。
- 既存の接続先解決（`--server` 未指定時のグローバル設定）と認証非対応の方針を踏襲する。

## Impact（影響範囲）
- Affected specs: `openspec/specs/cli/spec.md`
- Affected code: `src/cli.rs`, `src/main.rs`, `src/remote/client.rs`
