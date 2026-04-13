## Specification Tasks

- [ ] 1. 旧 `EventSink トレイトによるフロントエンド抽象化` (L3-16) を削除
  - Expected canonical result: `ReducerCommand` 条項を含む版のみが残る
  - verification: manual — spec diff で重複が 1 件になっていること
- [ ] 2. 重複する `## Requirements` ヘッダーを 1 つに統合
  - Expected canonical result: `## Requirements` が 1 つだけ
  - verification: integration — `openspec validate frontend-abstraction --strict` が通過
- [ ] 3. `## Purpose` セクションを追加
  - Expected canonical result: Purpose + Requirements の標準構造
  - verification: integration — `openspec validate frontend-abstraction --strict` が通過

## Future Work

- なし
