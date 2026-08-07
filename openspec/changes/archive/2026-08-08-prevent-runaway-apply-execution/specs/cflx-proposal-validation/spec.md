## ADDED Requirements

### Requirement: Proposal verification planはApply-owned workを有限化する

bundled proposal guidanceはApply-blocking verificationをrepository-localかつdefaultで1回のdirect executionに制限しなければならない（MUST）。Docker、database、heavy、credentialed、deployed-service、physical-device、external-approval、long-running repository-wide gateは、1 Apply invocation内で完了するbounded repository-local pathをproposalが宣言しない限り、repository automation、Acceptance、benchmark、manual、operational-observation ownershipへ割り当てなければならない（MUST）。proposal guidanceは同一verification commandのstability executionだけを目的とするcheckbox taskを作成してはならない（MUST NOT）。

#### Scenario: Heavy repository gateはApply checkboxにしない

- **GIVEN** changeがDockerとdatabaseを使用するrepository-wide validation suiteを必要とする
- **AND** requirement-specific repository-local testがintegration前にimplementationを証明できる
- **WHEN** bundled proposal guidanceがverification planを作成する
- **THEN** active implementation checkboxはbounded repository-local testを参照する
- **AND** heavy suiteはrepository automation、Acceptance、operational observationへ割り当てる
- **AND** heavy suiteの反復実行をcheckboxへ要求しない

#### Scenario: Bounded repository-local integration testはcompletionをblockできる

- **GIVEN** database behaviorを1つのdirect bounded commandとlocal fixtureで検証できる
- **WHEN** proposal guidanceがverificationを宣言する
- **THEN** `pre-integration`、`repository-local`、`change-blocking`として宣言できる
- **AND** taskは1つのrerun commandを指定しstability loopを要求しない

#### Scenario: Non-local verificationをtask proseへ隠さない

- **GIVEN** outcomeがcredential、deployment、physical hardware、external approvalを必要とする
- **WHEN** proposal guidanceがtaskとstructured verificationを書く
- **THEN** outcomeをApply-blocking checkboxへ紐付けない
- **AND** structured verification roleをoperational observationまたはnarrative Future Workとする
