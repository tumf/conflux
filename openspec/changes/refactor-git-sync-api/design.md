# Design: git sync API の責務分割

## 現状

`src/server/api/git_sync.rs` には command parsing/execution、sync planning、route handler、integration test setup が混在している。実 git repository を使うテストも同じファイル内に多く、読み込み時の認知負荷が高い。

## 方針

- API の outward contract は固定する。
- plan 判定と route orchestration を分離する。
- resolve command 実行は prompt 展開と shell 実行の contract を characterization test で守る。
- fixture はテスト専用 helper に寄せ、production path と混ぜない。

## 分割候補

- `resolve` command 実行とログ化。
- remote/local SHA 比較と sync plan。
- pull/push/sync route handler。
- bare repo / worktree fixture setup。

## Trade-offs

ファイル分割により module 境界は増えるが、git 操作の危険分岐を個別に検証しやすくなる。今回は安全なリファクタリングに限定し、Git コマンドの実行順序やエラー文言は意図的に変更しない。
