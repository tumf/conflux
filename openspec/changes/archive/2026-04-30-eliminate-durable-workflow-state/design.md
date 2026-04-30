## Why

現行の parallel resume / archive 制御は `~/.local/state/cflx/acceptance-state` と `~/.local/state/cflx/archive-resume-state` を workflow state として参照している。これは workspace の file/git state だけでは次アクションを決定できない hidden input を導入しており、workspace の可搬性・再現性・デバッグ可能性を損なう。

ユーザ方針は「workspace の状態は workspace だけで判定できなければならない」「durable state は完全禁止・有害」である。よって設計上は、workflow state machine の正本を workspace-local evidence へ閉じ込め、out-of-worktree data を制御入力から除去する必要がある。

## Design Goals

- resume routing と archive/acceptance handoff を workspace-local evidence のみで決定する
- `~/.local/state/cflx/**` の有無で同一 workspace の挙動が変わらないことを保証する
- `Applied` / `Archiving` / `Archived` 判定を file/git-state ベースに統一する
- observability と workflow control を責務分離し、外部ログ/metrics が残っても state machine は影響を受けないようにする

## Non-Goals

- serial mode 全体の刷新
- archive UX 文言の大規模改善
- 外部ログ保存そのものの禁止

## Proposed Design

### 1. Authoritative state inputs

workflow control が参照してよい入力を次に限定する。

- 対象 workspace 内の tracked/untracked file state
- 対象 workspace の git history / HEAD / index / working tree
- base branch HEAD tree との比較

次は workflow control input として禁止する。

- `~/.local/state/cflx/**`
- 別プロセスの in-memory history
- UI cache や log restore data
- 他 workspace に置かれた補助 state

### 2. Applied routing

`Applied` は「apply commit はあるが archive complete ではない」状態として維持する。

この状態から archive handoff してよいかは workspace-local evidence だけで決める。

- workspace 内に archive handoff readiness を十分に証明する evidence がある場合のみ archive へ進める
- その証明がない場合は acceptance を再実行する

この proposal では安全側の default を採り、workspace-local proof が不足するときは常に acceptance を再実行する。

### 3. Archiving / Archived routing

`Archiving` と `Archived` は既存 `src/execution/state.rs` の file/git-state detection を主系として維持する。

- `Archiving`: change directory が消え archive entry があり、commit completion が未完了
- `Archived`: archive completion が workspace-local evidence から確認できる

stale external state が存在しても、これらの判定や routing を変更してはならない。

### 4. Observability separation

archive retry reason や acceptance history のような観測情報は、残すとしても non-authoritative とする。

- logs/events/metrics は表示用・監査用には使える
- ただし resume routing、archive gate、acceptance gate の判定には使えない
- 外部 observability data が消えても runtime は正しく次アクションを決められなければならない

### 5. Migration / cleanup

実装では以下を行う。

- `src/parallel/acceptance_state.rs` と `src/parallel/archive_state.rs` の workflow-state responsibility を削除または到達不能化する
- 参照箇所 (`src/parallel/dispatch.rs`, `src/parallel/executor.rs`) を workspace-local routing へ切り替える
- regression tests で external state presence/absence が routing に影響しないことを固定する
- canonical spec から outside-the-worktree persistence 要求を削除する

## Verification Strategy

- workspace resume tests で `Applied`, `Archiving`, `Archived` の routing を file/git-state だけで確認する
- external durable state directory を事前作成/削除した両ケースで同一 outcome を assert する regression test を追加する
- `cflx openspec validate eliminate-durable-workflow-state --strict --evidence warn` を通す
- 実装時は targeted Rust tests と lint を実行し、external state 依存が残ると失敗する coverage を用意する
