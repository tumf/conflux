## MODIFIED Requirements

### Requirement: Bulk execution mark toggle reports complete target results

Changes view の bulk execution mark toggle は、操作開始時点の visible pre-archive proposal 全体を1つの対象集合として扱わなければならない（SHALL）。対象集合に未マークが1件でもあれば対象全件をマークし、対象全件がマーク済みなら対象全件をアンマークしなければならない（SHALL）。

Execution mode、active/retry/wait status、Apply iteration-limit evidence、および現在の parallel eligibility は対象集合から pre-archive proposal を除外する理由にしてはならない（MUST NOT）。`archived`、`merged`、および `pushed` proposal は対象集合へ含めてはならず（MUST NOT）、post-archive 除外だけを理由に warning を表示してはならない（MUST NOT）。

Bulk 操作は execution mark のみを変更しなければならない（SHALL）。Running mode を含め、`AddToQueue`、`RemoveFromQueue`、stop/dequeue、retry、resolve、cancellation、scheduler、hook、または process-mode effect を発行してはならない（MUST NOT）。結果は変更件数と共通 target mark state を報告し、対象集合が空の場合は mark mutation のない no-op として扱わなければならない（SHALL）。

#### Scenario: 部分的にマーク済みなら全 pre-archive proposal をマークする

**Given**: visible pre-archive proposal の一部だけが実行マーク済みである
**When**: ユーザーが Changes view で `x` を押す
**Then**: visible pre-archive proposal はすべて実行マーク済みになる
**And**: 既にマーク済みの proposal もマーク状態を維持する

#### Scenario: 全 pre-archive proposal がマーク済みなら全件をアンマークする

**Given**: visible pre-archive proposal がすべて実行マーク済みである
**When**: ユーザーが Changes view で `x` を押す
**Then**: visible pre-archive proposal はすべて未マークになる
**And**: current-run queue と active execution は変化しない

#### Scenario: lifecycle と eligibility が混在しても同じ mark target になる

**Given**: active、error、wait、Apply-limit、parallel-ineligible、および ordinary pre-archive proposal が混在する
**When**: ユーザーが Changes view で `x` を押す
**Then**: 全 visible pre-archive proposal に同じ target mark state が適用される
**And**: queue、runtime、retry、resolve、cancellation、scheduler、hook、および mode state は変化しない

#### Scenario: post-archive 行だけの場合は silent no-op

**Given**: 表示中の proposal がすべて archived、merged、または pushed である
**When**: ユーザーが Changes view で `x` を押す
**Then**: proposal の状態は変更されない
**And**: mark refusal warning は表示されない

<!-- Expected canonical result after archive: `tui-state-management` will define bulk marks over all visible pre-archive rows and remove Running queue-command and lifecycle exclusion semantics. -->
