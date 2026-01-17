## 1. Implementation
- [ ] 1.1 Worktree衝突チェックでmerge --abortを使わない判定手順に切り替える
- [ ] 1.2 git merge-treeを利用したコンフリクト判定の出力解析を実装する
- [ ] 1.3 TUIのWorktree更新フローで新しい判定結果を反映する
- [ ] 1.4 既存の衝突検出テスト/挙動を確認し必要なら更新する
- [ ] 1.5 既存の手順（Worktree作成/削除後の更新）を崩さないことを確認する

## 2. Validation
- [ ] 2.1 npx @fission-ai/openspec@latest validate avoid-merge-abort-conflict-check --strict
- [ ] 2.2 cargo test（関連ユニットテストのみ）
