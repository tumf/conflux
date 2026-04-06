## MODIFIED Requirements

### Requirement: キュー変更デバウンスとスロット駆動の再分析

並列実行中、システムはキュー変更（追加・削除）を実行中でも監視し、変更から10秒経過した後に再分析を行い、実行スロットが空いたタイミングで依存関係を考慮して次の変更を選定しなければならない（SHALL）。

加えて、システムは再分析時に実行スロットの空き数を算出し、依存関係分析の `order`（依存関係を満たした上での推奨実行順序）に従って空き数分の change を同時に起動しなければならない（SHALL）。

依存関係は実行制約として扱い、`order` の上位にあっても依存先が base に Git マージされた状態（依存先の成果物を使って実行できる状態）になるまで開始してはならない（MUST）。

依存制約が解決した change は、依存解決後の実行開始時点で worktree を新規作成し、既存の worktree がある場合も作り直さなければならない（MUST）。この挙動は依存 change に固有であり、resume が常に成立することを保証しない前提の例外とする。

#### Scenario: 空きスロット数に応じて同時起動する
- **GIVEN** `max_concurrent_workspaces` が 3 に設定されている
- **AND** 依存関係が解決済みの change が 3 件以上ある
- **WHEN** 再分析が実行される
- **THEN** システムは空きスロット数に応じて最大 3 件まで同時に起動する
- **AND** 依存関係が未解決の change は起動しない

#### Scenario: 後続 change でも dependency block が反映される
- **GIVEN** analyzer が `change-b` は `change-a` に依存すると返している
- **AND** `change-a` は base branch に未 merge である
- **AND** 現在の再分析で他の ready change が先に空きスロットを消費する
- **WHEN** scheduler が dispatch 対象と blocked state を更新する
- **THEN** `change-b` は起動されない
- **AND** `change-b` は dependency blocked として扱われる
- **AND** blocked 判定は available slot が残っているかどうかに依存しない
