## MODIFIED Requirements

### Requirement: Apply prompt MUST escalate implementation blockers

apply プロンプトは、仕様矛盾や非モック可能な外部制限により実装が不可能と判断した場合、Implementation Blocker を記録してエスカレーションしなければならない（MUST）。

Implementation Blocker の記録は以下を満たさなければならない（MUST）。
- `openspec/changes/{change_id}/tasks.md` に `## Implementation Blocker #<n>` セクションを追加する
- セクション内に「カテゴリ」「根拠（ファイルパス/ログ）」「影響範囲」「解除アクション」を明記する
- セクション内の箇条書きにチェックボックスを付けてはならない（MUST NOT）
- stdout に `IMPLEMENTATION_BLOCKER:` ブロックを出力し、tasks.md と同じ内容を含める
- recoverable blocker では terminal rejection artifact を生成せず、machine-readable apply outcome として `BLOCKED` を返す
- `REJECTED.md` を生成してよいのは、change 全体の reject 提案として recovery より closure が妥当である理由を apply が明示できる場合に限る

#### Scenario: apply が recoverable blocker を BLOCKED outcome として記録する

- **GIVEN** apply が仕様矛盾、fixture 不足、追加情報待ち、または依存未解消により現時点では実装を進められない
- **AND** blocker section に解除条件を書ける
- **WHEN** apply がエスカレーションを行う
- **THEN** tasks.md に `## Implementation Blocker #<n>` セクションが追加される
- **AND** stdout に `IMPLEMENTATION_BLOCKER:` ブロックが出力される
- **AND** apply outcome は `BLOCKED` として報告される
- **AND** worktree-local `REJECTED.md` は生成されない

#### Scenario: apply が terminal rejection proposal を明示的に区別する

- **GIVEN** apply が proposal の前提破綻や superseded 状態により change 全体を閉じるべきと判断する
- **WHEN** apply が rejection proposal を出す
- **THEN** stdout には recoverable blocker と区別された rejection proposal outcome が出力される
- **AND** worktree-local `REJECTED.md` 生成はこの outcome に限定される

#### Scenario: infrastructure verification blocker is not terminal rejection

- **GIVEN** apply or verification observes an external/local infrastructure failure such as Docker daemon unavailable, Docker image pull DNS timeout, package registry timeout, port conflict, third-party outage, or rate limiting
- **AND** no independent evidence shows that the proposal premise is invalid or obsolete
- **WHEN** the agent records the blocker
- **THEN** prompt guidance directs the agent to record a recoverable blocker or stalled hold
- **AND** prompt guidance does not direct the agent to create `REJECTED.md` as a terminal rejection marker for that infrastructure condition

### Requirement: Acceptance prompt MUST evaluate implementation blockers

acceptance プロンプトと配布 acceptance 関連 skill は Implementation Blocker を stalled acceptance hold として説明しなければならない（MUST）。妥当な blocker については、互換期間中の protocol handoff として `ACCEPTANCE: GATED` / `{"acceptance":"gated"}` を出力してもよい（MAY）。ただし、この verdict は user-facing lifecycle/status としての `gated` を意味してはならず、runtime は paused state を `stalled` として扱わなければならない（MUST）。parser が別 change で対応するまで、配布 guidance は final machine-readable verdict として `{"acceptance":"stalled"}` を出力するよう指示してはならない（MUST NOT）。

acceptance は以下を満たさなければならない（MUST）。
- `Implementation Blocker` の内容が不十分または誤りの場合は `ACCEPTANCE: FAIL` を出力し、follow-up タスクを tasks.md に追加する
- acceptance blocker verdict の場合は blocker の概要を簡潔に出力する
- repo 内編集・テスト更新・spec/tasks/proposal 修正など、AI がこの repository 内の変更だけで自律的に解決できる問題は `FAIL` として返さなければならない（MUST）。
- 人判断待ち、repo 外の設定変更、外部依存の解消待ち、追加情報待ち、または apply を再実行しても repository 変更だけでは解決不能な blocker は acceptance blocker verdict として返さなければならない（MUST）。
- blocker verdict を返さず `FAIL` を返した場合、その finding は apply へ戻って repository 作業を行うことで解消可能であることを意味しなければならない（MUST）。
- apply-generated recoverable blocker を審査するレビュー経路では、「change を reject するか」と「change を stalled hold のまま保留するか」を区別できなければならない
- 互換期間中に旧 `blocked` acceptance verdict や `gated` verdict を runtime が受理できても、新規 lifecycle/status contract は `stalled` を operator-facing term として使わなければならない（MUST）
- 配布 skill と command mirror は `gated` を primary rubric label や lifecycle/status 名として説明してはならず、stalled hold の互換 protocol token としてのみ説明しなければならない（MUST）

#### Scenario: acceptance emits blocker verdict for a valid implementation blocker

- **GIVEN** acceptance が妥当な Implementation Blocker を確認した
- **AND** その blocker は repository 内編集だけでは解決できない
- **WHEN** reviewer が machine-readable verdict を返す
- **THEN** verdict は stalled acceptance hold の compatibility handoff として解釈される
- **AND** 互換 token は現在の parser が受理する `{"acceptance":"gated"}` / `ACCEPTANCE: GATED` である
- **AND** runtime/user-facing status は `stalled` として扱われる
- **AND** blocker の概要が添えられる
- **AND** reviewer は parser support が入るまで `{"acceptance":"stalled"}` を出力しない

#### Scenario: acceptance uses fail for repository-fixable issues

- **GIVEN** acceptance が code / tests / tasks / spec の repository 内修正で解決できる問題を見つけた
- **WHEN** reviewer が machine-readable verdict を返す
- **THEN** verdict は `fail` を使う
- **AND** finding は apply が実行可能な repository-local follow-up として記録される

#### Scenario: acceptance uses stalled hold for infrastructure verification blocker

- **GIVEN** acceptance が必須 verification gate を実行した
- **AND** gate failed because of external/local infrastructure unavailability such as Docker image pull DNS timeout, Docker daemon unavailable, registry timeout, third-party outage, or missing non-mockable external credential
- **AND** repository-local code, tests, specs, or tasks cannot resolve that blocker
- **WHEN** reviewer emits a machine-readable verdict
- **THEN** verdict is the stalled-hold compatibility handoff `{"acceptance":"gated"}` / `ACCEPTANCE: GATED`
- **AND** guidance states this must be runtime/user-facing `stalled`, not terminal `rejected`
- **AND** guidance includes next action to restore the missing infrastructure or credential and rerun the gate

### Requirement: Rejecting review MUST distinguish terminal rejection from stalled blockers

Rejecting review prompt and distributed `cflx-rejecting` skill SHALL support three final verdicts: `REJECTION_REVIEW: CONFIRM`, `REJECTION_REVIEW: RESUME`, and `REJECTION_REVIEW: BLOCK`.

`CONFIRM` SHALL be used only when the change itself should be closed. `RESUME` SHALL be used when the rejection proposal is invalid, underspecified, or repository-fixable. `BLOCK` SHALL be used when the rejection proposal describes a real blocker but the change remains valid and resumable.

#### Scenario: rejecting review blocks infrastructure rejection proposal

- **GIVEN** worktree-local `REJECTED.md` proposes rejection because Docker image pull failed due DNS/network timeout
- **AND** the change validates structurally and remains active
- **WHEN** rejecting review classifies the proposal
- **THEN** the final marker is `REJECTION_REVIEW: BLOCK`
- **AND** guidance treats the condition as a non-terminal stalled hold
- **AND** guidance does not confirm terminal rejection

#### Scenario: rejecting review confirms invalid premise

- **GIVEN** worktree-local `REJECTED.md` proposes rejection with evidence that the proposal premise contradicts canonical specs or constitution
- **WHEN** rejecting review classifies the proposal
- **THEN** the final marker may be `REJECTION_REVIEW: CONFIRM`
- **AND** guidance allows runtime to create terminal base-branch `REJECTED.md`
