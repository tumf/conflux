# parallel-analysis Specification

## Purpose
TBD - created by archiving change pass-approved-changes-to-analyzer. Update Purpose after archive.
## Requirements

### Requirement: Parallel dependency analysis prompt

依存分析プロンプトは、`dependencies` に記載できる dependency target を **今回の queued change IDs** と **明示的に渡された in-flight change IDs** に限定しなければならない（MUST）。repo 上に存在するが今回の working set に含まれない active change、archived change、または単なる関連 change を dependency target として返してはならない（MUST NOT）。

#### Scenario: Reject dependency outside queued and in-flight working set
- **GIVEN** queued changes `beta`, `gamma` が dependency analysis 対象である
- **AND** in-flight change IDs は `alpha` のみである
- **WHEN** analyzer response が `gamma -> delta` の dependency を返す
- **THEN** runtime validation は parse error を返す
- **AND** error には `delta` が queued/in-flight working set 外の invalid dependency reference であることが含まれる

#### Scenario: Allow dependency to an in-flight change
- **GIVEN** queued changes `beta`, `gamma` が dependency analysis 対象である
- **AND** in-flight change IDs に `alpha` が含まれる
- **WHEN** analyzer response が `beta -> alpha` の dependency を返す
- **THEN** runtime validation はその dependency を受理する
- **AND** `alpha` は `order` に含まれないまま dependency target として扱われる
