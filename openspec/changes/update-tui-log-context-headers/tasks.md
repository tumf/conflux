## 1. 実装
- [ ] 1.1 TUIログ用イベントにoperation/iterationを追加し、analysis/resolve/archive/ensure_archive_commitへ伝播できるようにする
- [ ] 1.2 TUIログヘッダー表示を更新し、[operation:iteration] 形式の表示に統一する
- [ ] 1.3 apply/archive/ensure_archive_commit/analysis/resolveの出力イベント送信側を更新する
- [ ] 1.4 既存ログの互換性（change_idのみ/operationのみ）を維持する

## 2. 検証
- [ ] 2.1 TUIログのユニットテストを追加または更新する
- [ ] 2.2 cargo fmt
- [ ] 2.3 cargo clippy -- -D warnings
- [ ] 2.4 cargo test
