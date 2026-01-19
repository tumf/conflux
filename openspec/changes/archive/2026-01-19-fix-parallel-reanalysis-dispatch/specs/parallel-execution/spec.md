## MODIFIED Requirements
### Requirement: キュー変更デバウンスとスロット駆動の再分析
並列実行中、システムはキュー変更（追加・削除）を実行中でも監視し、変更から10秒経過した後に再分析を行い、実行スロットが空いたタイミングで依存関係を考慮して次の変更を選定しなければならない（SHALL）。

加えて、システムは再分析時に実行スロットの空き数を算出し、依存関係分析の `order`（依存関係を満たした上での推奨実行順序）に従って空き数分の change を同時に起動しなければならない（SHALL）。

実行スロットの空き数は「アクティブな change の数」を基準に計算しなければならない（MUST）。アクティブな change は apply / acceptance / archive / resolve が進行中の change とし、merged / merge_wait / error / not queued はアクティブとして扱ってはならない（MUST NOT）。

依存関係は実行制約として扱い、`order` の上位にあっても依存先が base に Git マージされた状態（依存先の成果物を使って実行できる状態）になるまで開始してはならない（MUST）。

依存制約が解決した change は、依存解決後の実行開始時点で worktree を新規作成し、既存の worktree がある場合も作り直さなければならない（MUST）。この挙動は依存 change に固有であり、resume が常に成立することを保証しない前提の例外とする。

#### Scenario: 実行中の空きスロットでキュー追加が起動する
- **GIVEN** `max_concurrent_workspaces` が 3 に設定されている
- **AND** 進行中（apply / acceptance / archive / resolve）の change が 2 件である
- **AND** 実行中にキューへ新しい change が追加される
- **AND** 追加された change の依存関係はすべて解決済みである
- **WHEN** 実行スロットが空いたタイミングを迎える
- **THEN** システムはバッチ完了を待たずに新しい change を起動する
- **AND** 起動は `order` に従い空きスロット数を超えない
