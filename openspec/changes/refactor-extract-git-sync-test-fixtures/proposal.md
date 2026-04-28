---
change_type: implementation
priority: medium
dependencies: []
references:
  - src/server/api/git_sync.rs
  - openspec/specs/testing/spec.md
  - openspec/specs/server-api/spec.md
---

# Change: git_sync 回帰テストのフィクスチャを抽出する

**Change Type**: implementation

## Premise / Context

- `src/server/api/git_sync.rs` は 2,000 行超の大型モジュールで、末尾には API 本体に加えて長い統合テストが同居している。
- `test_git_sync_runs_resolve_when_shas_differ` や `test_git_sync_runs_resolve_when_remote_ahead` では、registry / AppState / router 構築、ローカル bare repo の改変、scratch clone の作成などが繰り返し記述されている。
- これらのテストには `#[allow(clippy::await_holding_lock)]` や多数の `unwrap()` が現れ、シナリオの本質よりも準備コードが支配的になっている。
- 既存仕様では testing が「仕様ベーステスト」と「回帰検証の継続」を要求し、server-api では `git/sync` の公開挙動維持が要求されている。

## Problem / Context

git_sync の重要な分岐（同期済み、SHA 差異あり、remote ahead など）は既にテストされているが、現在のテストは fixture 構築と repo 変異の手順が各ケースに埋め込まれており、何を検証しているのかが読み取りづらい。将来ケースを追加するときにも、同じ準備コードを複製しやすく、期待 JSON や状態遷移の assertion が埋もれて回帰検知力が下がる。

本件は実運用コードではなくテスト構造の問題なので、公開 API を変えずに低リスクで改善できる。characterization test を先に固定したうえで、共通 fixture / helper 抽出によってシナリオを短く・意図中心に保つべきである。

## Proposed Solution

git_sync の回帰テストから、共有できる router/state/repo fixture を抽出し、シナリオ本体は期待挙動の assertion に集中させる。

- registry / AppState / router 初期化を共通 helper に抽出する
- origin / local bare / scratch clone を使った divergence 生成手順を helper 化する
- 現在の成功系・skip 系・resolve 実行系レスポンスを characterization test で固定する
- `status`、`resolve_command_ran`、`resolve_exit_code`、`push.status`、`skipped_reason` などの公開レスポンス項目は維持する
- 本番 API 実装のレスポンス形式・ログ意味論・HTTP ステータスは変更しない

## Acceptance Criteria

- `git/sync` の既存回帰シナリオは helper 抽出後も同じ HTTP ステータスと JSON フィールドを返す
- no-op sync、local/remote divergence、remote ahead などの主要シナリオが引き続き明示的に検証される
- テスト準備コードの重複が減り、シナリオ本文が期待挙動中心に読める構造になる
- 本番の `git_sync` API 実装の公開挙動に意図しない変更がない
- 追加ケースが共通 fixture を再利用して書ける状態になる

## Explicit Completion Conditions

- git_sync の主要回帰シナリオを先に固定する characterization test が追加または更新されている
- state / router / repo divergence 構築の共通 helper が抽出されている
- `src/server/api/git_sync.rs` のテスト本体から重複した準備コードが減っている
- `cargo test` が成功し、既存の server API 回帰が発生していない
- `cflx openspec validate refactor-extract-git-sync-test-fixtures --strict` が成功する

## Out of Scope

- `git_sync` 本体アルゴリズムの変更
- API レスポンス項目の追加・削除・名称変更
- remote sync policy や resolve_command 実行条件の変更
