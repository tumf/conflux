## ADDED Requirements

### Requirement: Dynamic queue ingestion resolves changes from the configured repository root

スケジューラの dynamic queue ingestion における候補 change の検証は、プロセスの
カレントディレクトリではなく、orchestrator/executor に設定された repository root を
基準に OpenSpec change を解決しなければならない（MUST）。`repo_root` がプロセス cwd と
異なる場合でも、ingestion の判定結果は `repo_root` 配下の `openspec/changes/` の内容
のみに依存しなければならない（MUST）。

候補 id が `repo_root` 配下の active change として存在しない場合、ingestion は既存の
`candidate_not_found` reconciliation ログを発行し、scheduler-local queued へ追加して
はならない（MUST NOT）。

この要件の回帰カバレッジは、ホストリポジトリ自身の OpenSpec change 内容（active /
archive 状態）に依存しない self-contained な fixture で検証可能でなければならない
（MUST）。

#### Scenario: Candidate present only under the configured repo root is ingested

- **GIVEN** executor が temp ディレクトリ R を `repo_root` として構成され、プロセス cwd は R と異なる
- **AND** change `synthetic-change` が R の `openspec/changes/synthetic-change/` にのみ存在する
- **WHEN** dynamic queue に `synthetic-change` が push され ingestion が評価される
- **THEN** `synthetic-change` は scheduler-local queued へ取り込まれる
- **AND** "Dynamically added to parallel execution" のログイベントが発行される

#### Scenario: Candidate absent under the configured repo root is not queued

- **GIVEN** executor が temp ディレクトリ R を `repo_root` として構成されている
- **AND** 候補 id `missing-change` が R 配下の active change として存在しない（プロセス cwd 配下の存在有無に関わらず）
- **WHEN** dynamic queue から `missing-change` が pop され ingestion が評価される
- **THEN** `missing-change` は scheduler-local queued へ追加されない
- **AND** `candidate_not_found` の reconciliation ログが発行される
