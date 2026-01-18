## 1. 仕様更新
- [ ] 1.1 CLI acceptance loop の未マーカー時 CONTINUE 仕様を追加する
- [ ] 1.2 Parallel acceptance loop の未マーカー時 CONTINUE 仕様を追加する

## 2. 実装
- [ ] 2.1 acceptance 出力パーサのデフォルト判定を CONTINUE に更新する
- [ ] 2.2 CLI acceptance loop の CONTINUE リトライに未マーカー判定を含める
- [ ] 2.3 Parallel acceptance loop の CONTINUE リトライに未マーカー判定を含める

## 3. 検証
- [ ] 3.1 acceptance パーサのユニットテストを追加/更新する
- [ ] 3.2 既存の acceptance 仕様に対する影響を確認する
- [ ] 3.3 `cargo test` を実行する

## 4. ドキュメント/ログ
- [ ] 4.1 acceptance 判定のデフォルトが CONTINUE であることを運用メモに反映する（必要なら）
