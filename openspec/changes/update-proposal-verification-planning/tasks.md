## Implementation Tasks

- [x] `skills/cflx-proposal/SKILL.md` の proposal planning guidance を更新し、behavior-changing requirement ごとに verification coverage planning を行う方針を追加する (verification: `skills/cflx-proposal/SKILL.md` に verification coverage planning の明示ルールが追加される)
- [x] `skills/cflx-proposal/SKILL.md` に標準 verification type vocabulary (`unit`, `integration`, `e2e`, `manual`, `benchmark`, `not-testable`) を追加する (verification: skill 文書内で各 verification type が proposal planning 用 vocabulary として列挙される)
- [x] `manual` と `benchmark` を intentional coverage として扱う guidance を追加し、unit test 非存在と未計画を区別できるようにする (verification: skill 文書で manual / benchmark が正当な verification path として説明される)
- [x] tasks.md guidance を更新し、verification note が verification ownership を追跡できるようにする (verification: `skills/cflx-proposal/SKILL.md` の tasks guidance で implementation task と verification path の対応が説明される)
- [ ] `python3 /Users/tumf/.agents/skills/cflx-proposal/scripts/cflx.py validate update-proposal-verification-planning --strict` を実行して proposal を検証する (verification: validation passed)

## Future Work

- proposal テンプレート例を verification type 付きで刷新する
- proposal 生成後の自動 lint/consistency check を将来追加する
