## MODIFIED Requirements

### Requirement: Future Work restrictions MUST be strictly enforced
Future Work への移動は、**人間の作業**、**外部システムのデプロイ/承認**、または**長時間待機が必要な検証**に限って許可されなければならない（MUST）。

面倒さ、難易度、テストの手間、回帰リスクなどを理由に Future Work へ移動してはならない（MUST NOT）。

#### Scenario: 人間作業や外部作業のみ Future Work へ移動する
- **GIVEN** tasks.md に人間作業や外部デプロイが必要なタスクがある
- **AND** tasks.md に難易度が高いが自動化可能なタスクがある
- **WHEN** apply エージェントがタスクの扱いを判断する
- **THEN** 人間作業や外部デプロイのタスクのみ Future Work に移動する
- **AND** 自動化可能なタスクは Future Work に移動しない
