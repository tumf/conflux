## 1. Implementation
- [ ] 1.1 worktree のアーカイブ検知を追加して archiving 状態を判定する
- [ ] 1.2 apply/archiving の遷移を更新し、archiving なら archive ループに進める
- [ ] 1.3 tasks.md が未完了または欠落している場合は archive を停止する
- [ ] 1.4 Future Work の許可条件を満たす場合のみ移動するようプロンプトを更新する
- [ ] 1.5 関連するログとエラーメッセージを更新する

## 2. Tests
- [ ] 2.1 archiving 状態の検知と遷移のテストを追加する
- [ ] 2.2 tasks 100% 未満時の archive ブロックを検証する
- [ ] 2.3 Future Work 指針が適用されることを確認する
