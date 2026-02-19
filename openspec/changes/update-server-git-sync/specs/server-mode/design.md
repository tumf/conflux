## Context
push/pull を個別に扱うとクライアントの分岐が増える。`git/sync` で 1 本化し、resolve を常に実行することで一貫性を高める。

## Design
- `git/sync` が pull → resolve → push の順で処理し、各結果をまとめて返す
- 既存の `git/pull` / `git/push` は互換ルートとして `sync` を呼び出す
