## MODIFIED Requirements
### Requirement: Apply Context History
オーケストレーターは、逐次/並列のどちらの apply でも共通ループで同一の履歴注入ロジックを使用し、各 apply 試行の最終サマリーメッセージを記録して同一 change の次回 apply プロンプトに含めなければならない（MUST）。さらに、acceptance が FAIL で apply ループへ戻る場合、次の apply 試行のプロンプトに直前の acceptance コマンド出力の stdout_tail/stderr_tail を `<last_acceptance_output>` ブロックで含めなければならない（MUST）。stdout_tail が空の場合は stderr_tail を使用し、両方空の場合はブロックを含めなくてもよい（MAY）。同一 acceptance 試行に由来する tail は最初の apply 試行にのみ注入し、以降の apply 試行では再注入してはならない（MUST NOT）。

#### Scenario: parallel の2回目 apply に履歴が含まれる
- **GIVEN** parallel mode で change が apply 実行中である
- **AND** 1回目の apply がエージェントのサマリーを返している
- **WHEN** 2回目の apply が実行される
- **THEN** プロンプトは base apply_prompt を含む
- **AND** プロンプトは `<last_apply attempt="1">` ブロックを含む
- **AND** ブロックには 1回目のサマリーが含まれる

#### Scenario: serial の2回目 apply に履歴が含まれる
- **GIVEN** 逐次モードで change が apply 実行中である
- **AND** 1回目の apply がエージェントのサマリーを返している
- **WHEN** 2回目の apply が実行される
- **THEN** プロンプトは base apply_prompt を含む
- **AND** プロンプトは `<last_apply attempt="1">` ブロックを含む
- **AND** ブロックには 1回目のサマリーが含まれる

#### Scenario: acceptance failure tail が次の apply に含まれる
- **GIVEN** acceptance が FAIL で終了して apply ループへ戻る
- **AND** AcceptanceHistory に stdout_tail または stderr_tail が記録されている
- **WHEN** 次の apply 試行が開始される
- **THEN** apply プロンプトは `<last_acceptance_output>` ブロックを含む
- **AND** stdout_tail が存在する場合は stdout_tail が含まれる
- **AND** stdout_tail が空の場合は stderr_tail が含まれる

#### Scenario: acceptance tail は 1 回だけ注入される
- **GIVEN** acceptance が FAIL で終了して apply ループへ戻る
- **AND** AcceptanceHistory に stdout_tail または stderr_tail が記録されている
- **WHEN** 連続して 2 回の apply 試行が実行される
- **THEN** 1 回目の apply プロンプトにのみ `<last_acceptance_output>` ブロックが含まれる
