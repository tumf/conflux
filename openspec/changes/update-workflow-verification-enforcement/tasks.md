## Implementation Tasks

- [ ] `skills/cflx-workflow/SKILL.md` の apply guidance を更新し、planned verification path と実 evidence の整合性を task completion 条件に含める (verification: apply completion guidance に verification type と evidence type の整合条件が記載される)
- [ ] `skills/cflx-workflow/SKILL.md` の accept guidance を更新し、`manual` / `benchmark` / `not-testable` を intentional coverage として扱うルールを追加する (verification: accept guidance で自動テスト不在と intentional coverage が区別される)
- [ ] `skills/cflx-workflow/SKILL.md` に unit-vs-integration mismatch の handling を追加する (verification: unit を主張する task に integration-style evidence しかない場合の follow-up 方針が記載される)
- [ ] proposal planning との接続を skill に明記し、verification type 未計画・曖昧時の acceptance finding 方針を追加する (verification: workflow skill に planning/enforcement の接続が記述される)
- [ ] `python3 /Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py validate update-workflow-verification-enforcement --strict` を実行して proposal を検証する (verification: validation passed)

## Future Work

- verification type を machine-readable に受け渡す proposal/task フォーマットを将来標準化する
- acceptance findings を verification mismatch 専用カテゴリで整理する
