## ADDED Requirements

### Requirement: Bulk execution mark toggle reports complete target results

Changes viewのbulk execution mark toggleは、操作開始時点のeligibleなproposal全体を1つの対象集合として扱わなければならない（SHALL）。対象集合に未マークが1件でもあれば対象全件をマークし、対象全件がマーク済みなら対象全件をアンマークしなければならない（SHALL）。

既存の安全guardによりactive、rejected、またはparallel-ineligibleなproposalは対象集合へ含めてはならない（MUST NOT）。除外行が存在する場合、TUIは操作された件数、除外された件数、およびユーザーが理解または対処できる除外理由を表示しなければならない（SHALL）。対象集合が空の場合も無反応にしてはならない（MUST NOT）。

Running modeのeligibleなqueue-mutating rowには、単一行のSpace操作と同じAddToQueue/RemoveFromQueue semanticsを適用しなければならない（SHALL）。active rowをbulk操作から停止要求へ変換してはならない（MUST NOT）。

#### Scenario: 部分的にマーク済みならeligible全件をマークする

**Given**: eligibleなproposalの一部だけが実行マーク済みである
**When**: ユーザーがChanges viewで`x`を押す
**Then**: eligibleなproposalはすべて実行マーク済みになる
**And**: 既にマーク済みのproposalもマーク状態を維持する

#### Scenario: eligible全件がマーク済みなら全件をアンマークする

**Given**: eligibleなproposalがすべて実行マーク済みである
**When**: ユーザーがChanges viewで`x`を押す
**Then**: eligibleなproposalはすべて未マークになる

#### Scenario: eligibleとineligibleが混在する

**Given**: eligibleな未マークproposalと、active、rejected、またはparallel-ineligibleなproposalが混在する
**When**: ユーザーがChanges viewで`x`を押す
**Then**: eligibleなproposalはすべてマークされる
**And**: ineligibleなproposalのマーク状態は変更されない
**And**: TUIは変更件数、除外件数、および除外理由を表示する

#### Scenario: bulk対象が存在しない

**Given**: 表示中のproposalがすべてbulk toggleの対象外である
**When**: ユーザーがChanges viewで`x`を押す
**Then**: proposalの状態は変更されない
**And**: TUIは対象がない理由を表示する

#### Scenario: Running modeでqueue commandを全対象へ発行する

**Given**: Running modeで複数のeligibleな`not queued` proposalが未マークであり、active proposalも存在する
**When**: ユーザーがChanges viewで`x`を押す
**Then**: 各eligibleな`not queued` proposalがマークされ、それぞれにAddToQueue commandが発行される
**And**: active proposalには停止commandもstate changeも発生しない
