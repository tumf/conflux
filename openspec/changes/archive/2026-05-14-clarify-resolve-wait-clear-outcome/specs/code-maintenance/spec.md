## MODIFIED Requirements

### Requirement: リファクタリング安全性の担保

オーケストレーターはリファクタリング後も既存仕様の挙動を保ち、検証手順で後退がないことを示すために SHALL 検証を通過しなければならない。OpenSpec コマンドエンジンの責務分離では、list/show/validate/archive の CLI contract と spec promotion の結果を characterization test で固定しなければならない。状態 reducer の副作用整理では、terminal/activity/wait state と resolve/reject wait queue の組み合わせを characterization test で固定しなければならない。加えて、タスク進捗解決のリファクタリングでは、worktree/base/archive の探索順序と acceptance follow-up の更新結果を characterization test で固定しなければならない。

ResolveWait retry helper naming and comments SHALL reflect the full set of outcomes that clear scheduler-owned retry membership. A helper used for successful merge, already-merged detection, missing workspace cleanup, and stale workspace cleanup MUST NOT be named or documented as success-only in a way that suggests stale cleanup is a successful merge.

<!-- Expected canonical result after archive: `code-maintenance` will require helper names/comments around retry intent clearing to describe outcome semantics accurately when used by both success and stale-cleanup paths. -->

#### Scenario: OpenSpec validate の contract が維持される

- **GIVEN** 妥当な変更提案と不正な変更提案が存在する
- **WHEN** strict validation を実行する
- **THEN** proposal、tasks、spec delta、scenario、change type の必須チェック結果はリファクタリング前と同等である
- **AND** exit code とエラー/警告の分類は同等である

#### Scenario: archive 前 promotion safety が維持される

- **GIVEN** ADDED、MODIFIED、REMOVED、または no-op になる spec delta が存在する
- **WHEN** archive 前の promotion simulation を実行する
- **THEN** canonical spec へ適用可能な delta だけが成功する
- **AND** missing target や no-op promotion は既存と同じ安全側の失敗として扱われる

#### Scenario: reducer command の状態遷移が維持される

- **GIVEN** 既存の `OrchestratorState` と代表的な `ReducerCommand` がある
- **WHEN** command を適用する
- **THEN** `ReduceOutcome`、terminal state、activity state、wait state、queue intent はリファクタリング前と同等である
- **AND** resolve/reject wait queue の更新結果も同等である

#### Scenario: execution event の副作用が維持される

- **GIVEN** archive、merge、rejection review、resolve failure の代表的な `ExecutionEvent` がある
- **WHEN** event を reducer に適用する
- **THEN** display status と global invariant はリファクタリング前と同等である
- **AND** base-mutating lane の待機 queue は既存と同じ条件で追加・削除される

#### Scenario: タスク進捗解決の探索順序が維持される

- **GIVEN** worktree active、worktree archive、base archive、base active の各 `tasks.md` 候補が存在する
- **WHEN** タスク進捗を fallback 付きで解析する
- **THEN** 既存と同じ優先順位で最初に見つかった `tasks.md` が使われる
- **AND** 解析された完了数と総数はリファクタリング前後で同じである

#### Scenario: acceptance follow-up の更新挙動が維持される

- **GIVEN** acceptance follow-up section を含む、または含まない `tasks.md` が存在する
- **WHEN** acceptance failure findings を記録する
- **THEN** 既存 section は同じ見出し単位で置換される
- **AND** 空 findings の場合は既定の未完了タスクが追加される

#### Scenario: ResolveWait clear helper describes outcome semantics

- **GIVEN** a helper clears scheduler-owned `ResolveWait` membership for success and stale cleanup paths
- **WHEN** a developer reads the helper name or adjacent comments
- **THEN** the code communicates that the helper applies to no-longer-retryable outcomes, not only successful merges
- **AND** stale or missing workspace cleanup is not described as merge success
