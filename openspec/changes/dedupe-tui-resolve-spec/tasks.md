## Specification Tasks

- [ ] 1. `openspec/specs/tui-resolve/spec.md` の古い `auto-resumable-merge-deferred-triggers-resolve` (L3-17, L37-57) を削除
  - Expected canonical result: Project スコープ版のみが残る
  - verification: manual — spec diff で重複が 1 件になっていること
- [ ] 2. 古い `resolve-merge-exclusive-execution` (L20-34) を削除
  - Expected canonical result: Project スコープフラグ条項を含む版のみが残る
  - verification: manual — spec diff で重複が 1 件になっていること
- [ ] 3. `## Purpose` セクションを追加
  - Expected canonical result: Purpose + Requirements の標準構造
  - verification: integration — `openspec validate tui-resolve --strict` が通過

## Future Work

- なし
