---
change_type: implementation
priority: high
dependencies: []
references:
  - src/analyzer.rs
  - src/parallel_run_service.rs
  - src/agent/runner.rs
  - src/templates.rs
  - openspec/specs/parallel-analysis/spec.md
  - openspec/specs/agent-prompts/spec.md
---

# Change: analyze JSON dependency contract を queued/in-flight 境界に合わせて修正する

**Change Type**: implementation

## Premise / Context

- 直近3時間の cflx 実行ログでは、dependency analysis が exit code 0 で JSON を返したにもかかわらず、`persist-archive-resume-reasons` が今回の queued set に含まれていない dependency を返したため `Invalid dependency reference` として parse error になり、parallel 実行が全件並列 fallback へ落ちていた（`~/.local/state/cflx/logs/conflux-bda270b8/2026-04-29.log:459165`）。
- archived change `update-analyze-use-proposal-frontmatter` は frontmatter `dependencies` / `priority` / `references` の利用規則を既に定義しており、frontmatter を dependency source として使う基盤自体は別途整理済みである（`openspec/changes/archive/update-analyze-use-proposal-frontmatter/proposal.md:16-24`）。
- 現行 runtime は `order` に含まれない dependency を in-flight IDs に対してのみ許容しており、それ以外は hard parse error として扱う（`src/analyzer.rs:627-657`）。
- 残っているギャップは、working set 外 dependency を返してはならない closed-world 契約が prompt / skill / diagnostics で十分に明文化されていない点である。

## Requested Artifact

- implementation proposal for tightening analyze prompt/skill/diagnostics around the existing queued-vs-in-flight dependency contract
- explicit rule for what dependency IDs may legally appear in `dependencies`
- regression coverage proving valid queued/in-flight references pass and unrelated active references fail with actionable diagnostics

## Problem / Context

Conflux の analyze phase は queued changes の実行順と hard dependency を JSON で受け取るが、現在の user/agent-facing 契約では「dependency target として返してよい change ID の集合」が十分に限定されていない。そのため agent は repo 上の他の active change や過去に見かけた change を dependency に返しうる。

実装上は queued set と in-flight set だけを合法な dependency target として扱っているため、範囲外 ID が返ると parse error になり、dependency analysis 全体が `run all changes in parallel` fallback へ落ちる。つまり新しい dependency model 実装が必要なのではなく、既存 contract の明文化と diagnostics 強化が不足している。

## Proposed Solution

analyze contract を「dependency target は今回の queued set または明示的に渡された in-flight set のみ」という existing rule に揃えて、prompt・skill・parser error message・tests を補強する。

- Rust-side analyze prompt は、`dependencies` に書ける change ID が queued IDs と in-flight IDs に限定されることを明示し、repo 上の他 active change や単なる関連 change を dependency に書いてはならないと強く指示する。
- dedicated `cflx-analyze` guidance と canonical spec を更新し、frontmatter 利用規則の上に載る dependency target contract を queued/in-flight working set ベースで明文化する。
- parser failure message は、invalid dependency reference 時に「allowed IDs」「queued IDs」「in-flight IDs」を含む actionable error にする。
- fallback 自体は維持しても、invalid JSON / invalid dependency の種別がログと UI で明確に分かるようにし、単なる generic invalid JSON と混同しない。
- regression tests で、(1) queued dependency、(2) in-flight dependency、(3) unrelated active dependency の3系統を固定する。

## Acceptance Criteria

- dependency analysis prompt と `cflx-analyze` guidance が、`dependencies` に書ける change ID を queued set と in-flight set に限定する existing closed-world rule を明示する。
- queued change が in-flight change に依存する JSON は引き続き受理される。
- queued でも in-flight でもない active change を dependency に含む JSON は parse error になるが、error message から allowed set と違反 ID が分かる。
- LLM analysis failure log は generic な `invalid JSON` だけで終わらず、invalid dependency reference が root cause だと分かる。
- regression tests が `persist-archive-resume-reasons -> align-archive-readiness-failure-reporting` 型の範囲外 dependency を再現し、fallback を起こす前に actionable diagnostics を残すことを確認する。

## Explicit Completion Conditions

- `openspec/specs/parallel-analysis/spec.md` と `openspec/specs/agent-prompts/spec.md` に dependency target の許容集合が canonical rule として記述され、archived `update-analyze-use-proposal-frontmatter` の frontmatter rule と矛盾しない。
- `src/analyzer.rs` の prompt builder / parser validation が existing allowed dependency set を同じルールで扱うよう tasks に明記されている。
- `src/parallel_run_service.rs` または関連ログ整形経路で invalid dependency failure の見え方を改善する task が含まれている。
- queued/in-flight/invalid-active の3種類をカバーする unit or integration tests が tasks に含まれている。
- `cflx openspec validate fix-analyze-json-dependency-contract --strict --evidence warn` が成功する。

## Out of Scope

- dependency analysis を完全 deterministic / non-LLM 化すること
- frontmatter metadata source そのものの再設計
- queued selection algorithm 全面改修
- apply/archive stall policy そのものの変更
