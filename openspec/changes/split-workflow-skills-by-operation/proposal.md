---
change_type: hybrid
priority: medium
dependencies: []
references:
  - skills/cflx-workflow/SKILL.md
  - src/agent/prompt.rs
  - src/orchestration/rejection.rs
  - src/embedded_skills.rs
  - skills/README.md
---

# Change: cflx-workflow を操作別スキルへ分割する

**Change Type**: hybrid

## Problem / Context

現在の `cflx-workflow` は apply / rejecting / cleanup-review / accept / archive の複数 operation を 1 つのスキルに同居させている。そのため orchestrator が operation を明示しているにもかかわらず、各 operation 実行時に不要なルールまで同じ SKILL.md に含まれ、プロンプトの文脈効率と保守性が低下している。

実装上も `src/agent/prompt.rs` および `src/orchestration/rejection.rs` は operation ごとの prompt を構築しているが、読み込むスキル名は一律 `cflx-workflow` のままである。これにより apply 実行時にも acceptance の厳格な verdict 規約が混入しうるなど、operation ごとの思考モード分離が不十分になっている。

一方で既存環境や既存 prompt 互換性のため、`cflx-workflow` 自体はすぐに削除せず、薄い互換ルータとして残す必要がある。

## Proposed Solution

- `skills/` 配下に operation 専用スキル `cflx-apply` / `cflx-rejecting` / `cflx-cleanup-review` / `cflx-accept` / `cflx-archive` を追加する
- 各 operation 専用スキルは、その operation に必要な auxiliary files を自分の skill directory 配下へ持つ
- 既存 `cflx-workflow` は互換性維持用の self-contained な互換ルータへ縮約し、legacy prompt が追加の skill load や cross-skill auxiliary file 参照なしで従来同等に apply / rejecting / cleanup-review / accept / archive を継続実行できるようにする
- orchestrator の prompt builder は operation に応じて個別スキル名を直接読み込むよう変更する
- `cflx-accept` は operation identity と補助 guidance を提供するが、固定 acceptance 手順の単一ソースは引き続き `.opencode/commands/cflx-accept.md` に維持する
- 埋め込みスキル登録と `install-skills` 配布対象を新しい個別スキル群込みへ更新する
- skills README と関連仕様を更新し、Conflux workflow スキル構成が router + operation-specific skills に変わったことを明示する

## Acceptance Criteria

- apply prompt は `cflx-apply`、archive prompt は `cflx-archive`、cleanup-review prompt は `cflx-cleanup-review`、acceptance prompt は `cflx-accept`、rejecting review prompt は `cflx-rejecting` を直接読み込む
- `cflx-workflow` は削除されず、旧 prompt が `load skills: cflx-workflow` を使っても追加の skill load や cross-skill auxiliary file 参照なしで従来同等に apply / rejecting / cleanup-review / accept / archive を継続実行できる self-contained な互換ルータとして残る
- operation 固有の詳細手順は個別スキルに移り、`cflx-workflow` 本体は legacy compatibility を守る最小十分な共通原則と導線中心の記述になる
- Conflux に埋め込まれる bundled skills と `cflx install-skills` の配布結果に、新しい operation 別スキル群が含まれ、各 skill は自分に必要な auxiliary files を同梱する
- `cflx-accept` 追加後も固定 acceptance 手順の単一ソースは `.opencode/commands/cflx-accept.md` に維持される
- テストは prompt builder / embedded skills / install-skills の新構成に追随し、旧 `cflx-workflow` 一括前提の assertion が残らない

## Out of Scope

- operation をさらに phase 単位へ細分化したスキル分割
- orchestrator に operation 自動判定ロジックを再導入すること
- `cflx-proposal` や `cflx-run` の役割変更
