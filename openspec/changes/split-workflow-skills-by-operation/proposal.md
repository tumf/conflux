---
change_type: hybrid
priority: medium
dependencies:
  - replace-cflx-py-with-native-cli
references:
  - skills/cflx-workflow/SKILL.md
  - src/agent/prompt.rs
  - src/orchestration/rejection.rs
  - src/orchestration/selection.rs
  - src/parallel/conflict.rs
  - src/embedded_skills.rs
  - skills/README.md
  - openspec/changes/replace-cflx-py-with-native-cli/proposal.md
---

# Change: cflx-workflow を操作別スキルへ分割する

**Change Type**: hybrid

## Problem / Context

現在の Conflux では operation ごとの prompt source が二系統に分かれている。workflow 系 operation（apply / rejecting / cleanup-review / accept / archive）は `cflx-workflow` に集約されている一方、analyze は `src/orchestration/selection.rs` の inline prompt、resolve は `src/parallel/conflict.rs` の inline prompt によって固定手順が Rust 実装へ埋め込まれている。そのため orchestrator が operation を明示していても、operation ごとの思考モード・固定ルール・補助資料の置き場が一貫していない。

実装上も `src/agent/prompt.rs` および `src/orchestration/rejection.rs` は workflow 系 prompt を構築しているが、読み込むスキル名は一律 `cflx-workflow` のままである。これにより apply 実行時にも acceptance の厳格な verdict 規約が混入しうるなど、operation ごとの思考モード分離が不十分になっている。さらに analyze / resolve は skill surface 自体を持たないため、固定 prompt ルールを skill として再利用・配布・更新しづらい。

加えて、進行中の `replace-cflx-py-with-native-cli` proposal は skill-local Python helper 依存を原則として native CLI へ移す方針である。その前提では、新しく追加する operation-specific skills は `cflx.py` を前提にせず、legacy compatibility を維持する `cflx-workflow` のみを例外扱いする必要がある。

一方で既存環境や既存 prompt 互換性のため、`cflx-workflow` 自体はすぐに削除せず、legacy compatibility router として残す必要がある。`cflx-workflow` に限っては既存互換 surface を守るため `scripts/cflx.py` を例外的に維持する。

## Proposed Solution

- `skills/` 配下に operation 専用スキル `cflx-analyze` / `cflx-apply` / `cflx-rejecting` / `cflx-cleanup-review` / `cflx-accept` / `cflx-archive` / `cflx-resolve` を追加する
- analyze / resolve を含む各 operation 専用スキルは、その operation に必要な固定 guidance と auxiliary references を自分の skill directory 配下へ持つ
- 新規 operation-specific skills は `replace-cflx-py-with-native-cli` の native CLI 方針に合わせて `cflx.py` を同梱せず、必要な OpenSpec 操作は `cflx openspec ...` 前提で記述する
- 既存 `cflx-workflow` は互換性維持用の self-contained な互換ルータへ縮約し、legacy prompt が追加の skill load や cross-skill auxiliary file 参照なしで従来同等に apply / rejecting / cleanup-review / accept / archive を継続実行できるようにする
- `cflx-workflow` に限っては legacy compatibility を守るため `scripts/cflx.py` を例外的に維持する
- orchestrator の prompt builder は operation に応じて個別スキル名を直接読み込むよう変更し、analyze / resolve の固定 prompt ルールも専用 skill 側へ移す
- `cflx-accept` は operation identity と補助 guidance を提供するが、固定 acceptance 手順の単一ソースは引き続き `.opencode/commands/cflx-accept.md` に維持する
- 埋め込みスキル登録と `install-skills` 配布対象を新しい個別スキル群込みへ更新し、`scripts/cflx.py` は `cflx-workflow` のみへ残す
- skills README と関連仕様を更新し、Conflux の operation prompt surface が router + dedicated operation skills に変わったことを明示する

## Acceptance Criteria

- analyze prompt は `cflx-analyze`、apply prompt は `cflx-apply`、archive prompt は `cflx-archive`、cleanup-review prompt は `cflx-cleanup-review`、acceptance prompt は `cflx-accept`、rejecting review prompt は `cflx-rejecting`、resolve prompt は `cflx-resolve` を直接読み込む
- `cflx-workflow` は削除されず、旧 prompt が `load skills: cflx-workflow` を使っても追加の skill load や cross-skill auxiliary file 参照なしで従来同等に apply / rejecting / cleanup-review / accept / archive を継続実行できる self-contained な互換ルータとして残る
- operation 固有の詳細手順は個別スキルに移り、`cflx-workflow` 本体は legacy compatibility を守る最小十分な共通原則と導線中心の記述になる
- `scripts/cflx.py` は `cflx-workflow` にのみ残り、その他の dedicated skills には含まれない
- Conflux に埋め込まれる bundled skills と `cflx install-skills` の配布結果に、新しい operation 別スキル群が含まれ、各 skill は自分に必要な auxiliary references を同梱する
- `cflx-accept` 追加後も固定 acceptance 手順の単一ソースは `.opencode/commands/cflx-accept.md` に維持される
- analyze / resolve を含む fixed prompt guidance は Rust 実装の inline text から dedicated skill source へ移り、Rust 側は可変コンテキスト注入に集中する
- テストは prompt builder / embedded skills / install-skills の新構成に追随し、旧 `cflx-workflow` 一括前提や inline analyze / resolve prompt 前提の assertion が残らない

## Out of Scope

- operation をさらに phase 単位へ細分化したスキル分割
- orchestrator に operation 自動判定ロジックを再導入すること
- `cflx-proposal` の役割変更
- `cflx-workflow` 以外の skill へ `cflx.py` 互換 helper を広げること
