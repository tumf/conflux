---
change_type: implementation
priority: high
dependencies: []
references:
  - src/analyzer.rs
  - src/openspec_cmd.rs
  - openspec/specs/parallel-execution/spec.md
  - ~/.local/state/cflx/logs/log-startup-version-88179737/2026-04-29.log
---

# Change: archived dependency references を analyze で正しく扱う

**Change Type**: implementation

## Problem/Context

`~/.local/state/cflx/logs/log-startup-version-88179737/2026-04-29.log` では、active change が archived change `separate-apply-block-from-reject` を metadata dependency として参照した結果、analyze が `Invalid dependency reference` を返し、最終的に `Analysis returned invalid JSON` 系のエラーとして実行が止まった。

Canonical spec は `openspec/specs/parallel-execution/spec.md` で archived dependency reference を generic JSON parse failure として潰してはならず、archived と missing を区別することを要求している。現行 `src/analyzer.rs` は診断に `dependency_target_classification={... class:'archived'}` を付与するが、依然として parse failure として返すため、archived dependency を明示的に satisfied/non-queued として扱う選択肢が実装されていない。

## Proposed Solution

Analyze結果の dependency validation で、dependency target が queued/in-flight/archived/missing のどれかを事前分類する。Archived dependency は ordering edge としては除外または satisfied と扱い、analyze 全体を失敗させない。Missing dependency は引き続き専用の invalid dependency reference error として失敗させる。

User-facing error text と logs は archived dependency を malformed JSON と区別して報告する。LLM が archived dependency を dependencies map に含めても、runtime 側で safe normalization して queued/in-flight edges のみを残す。

## Acceptance Criteria

- Active queued changes が archived-only dependency を参照していても、analyze は generic JSON parse failure で停止しない。
- Archived dependency targets は queued/in-flight dependency graph から除外されるか、明示的に satisfied として扱われる。
- Missing dependency targets は引き続き analyze failure になり、diagnostics は missing と archived を区別する。
- Regression test が、archived dependency を含む analysis JSON が成功し、missing dependency を含む analysis JSON が失敗することを検証する。

## Explicit Completion Conditions

- `src/analyzer.rs` の dependency validation/normalization が archived dependency を terminal parse error にしない。
- `cargo test analyzer` または同等の analyzer unit tests が archived/missing の両ケースを通す。
- `cflx openspec validate fix-archived-dependency-analyze-handling --strict --evidence warn` が成功する。

## Out of Scope

- Archived dependency metadata の自動削除や proposal frontmatter の書き換え。
- `cflx openspec validate` の archived dependency warning policy の変更。
- LLM analyze prompt の全面再設計。
