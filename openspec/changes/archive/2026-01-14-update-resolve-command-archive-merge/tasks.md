## 1. 仕様・設計
- [x] 1.1 `parallel-execution` の archive/merge 補正スコープを更新する
- [x] 1.2 archive コミットの成功条件と resume 判定条件を明文化する

## 2. 実装
- [x] 2.1 archive フェーズのコミットを resolve_command 経由に置き換える
- [x] 2.2 merge フェーズの change_id を OpenSpec の `change_id` に正規化する
- [x] 2.3 resume 時の archived 判定を「archive コミット済み」に強化する

## 3. テスト
- [x] 3.1 pre-commit 中断を模擬し、archive コミットが再試行で完了することを検証する
- [x] 3.2 merge コミットに `Merge change: <change_id>` が含まれることを検証する

## 4. 検証
- [x] 4.1 `cargo test` を実行する
- [x] 4.2 `cargo clippy` と `cargo fmt --check` を実行する
