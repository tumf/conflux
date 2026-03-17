# Change: push/pull を git/sync API に統合する

## Why
push/pull を個別に呼び分けるとクライアントの分岐が増え、実行結果の可視化も複雑になるため。`sync` を 1 本化し、resolve を必須化して動作を安定させる。

## What Changes
- `POST /api/v1/projects/{id}/git/sync` を追加し、push/pull の実行を 1 本化する
- `sync` は常に resolve_command を実行し、失敗時は同期失敗を返す
- 既存 `git/pull` / `git/push` は `sync` へ委譲する（互換性維持）

## Impact
- Affected specs: `openspec/specs/server-mode/spec.md`
- Affected code: `src/server/api.rs`, `src/server/runner.rs`, `src/remote/client.rs`
