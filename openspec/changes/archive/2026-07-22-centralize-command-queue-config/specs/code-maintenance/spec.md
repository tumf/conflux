## ADDED Requirements

### Requirement: Command runner configuration has one canonical construction path

現役のagent実行経路は、`OrchestratorConfig` からcommand queueとAI command runnerへ設定を反映する共通構築処理を使用し、経路ごとの設定転記を避けなければならない。共通化は既存のfallback、override、shared stagger state、command実行semanticsを変更してはならない。

#### Scenario: All queue settings are preserved by the common conversion

**Given**: queue、retry、timeout、cleanupの各設定に識別可能な値を持つ `OrchestratorConfig` がある
**When**: 共通構築処理が `CommandQueueConfig` を生成する
**Then**: 全queue設定フィールドはリファクタ前と同じ値になる
**And**: 未指定値にはリファクタ前と同じdefaultが使用される

#### Scenario: Configured runner preserves auxiliary settings

**Given**: stream JSON textification、strict process cleanup、command environmentsが設定されている
**When**: 共通構築処理が `AiCommandRunner` を生成する
**Then**: 生成されたrunnerは各設定を現在と同じように保持する
**And**: 呼び出し元から渡されたshared stagger stateを引き続き使用する

#### Scenario: Intentional overrides remain explicit

**Given**: test fixtureまたは特殊経路がtimeoutやretry値を意図的に上書きしている
**When**: productionの重複初期化が共通化される
**Then**: 意図的なoverrideは共通defaultへ置換されない
**And**: command実行結果とretry、timeout、cleanupの挙動はリファクタ前と同等である
